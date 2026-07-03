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
// C strategy names carry mixed-case part suffixes (boss7launcherT,
// bossA_cupL, spacepilonP, ...) preserved verbatim for function-for-function
// parity with strat_enemy.c.
#![allow(non_snake_case)]

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
// The eb_compat facade re-exports the shared helper surface for BOTH the
// enemy_b lane and the bosses lane (`bosses.rs` does `use eb_compat::*`).
// Not every re-export is consumed by enemy_b itself, so unused-import lints
// are silenced here — the facade is the contract, not a private import list.
#[allow(unused_imports)]
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

    // Canonical strat_common.c ports (crate::common). Short aliases (make_obj,
    // spawn_projectile, chase_proportional, speed_to, gen_vecs_3d, ...) match
    // the bosses lane's call sites.
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
    pub use crate::common::{
        strat_angle_xz as angle_xz, strat_apply_velocity as apply_velocity,
        strat_chase as chase, strat_chase_proportional as chase_proportional,
        strat_count_down as count_down, strat_gen_vecs_3d as gen_vecs_3d,
        strat_make_obj as make_obj, strat_spawn_projectile as spawn_projectile,
        strat_speed_to as speed_to,
    };

    // Canonical enemy_a front-half exports (landed).
    pub use crate::enemy_a::{
        bossflags, currentlevel, gasflags, pviewposz, set_bossflags, set_gasflags,
        strat_explode, strat_hit_flash, wm as ea_wm,
    };

    // enemy_a shared boss machinery + helpers (C strat_enemy.c:800-1296 and
    // the EXPSTRAT block — ported by the enemy_a lane, pub(crate) exports).
    pub(crate) use crate::enemy_a::{
        achase_angle, add_player_z, addrnd2pos_xy, boss7hatchfire_srou,
        boss7launcher_fire_hmissile1, boss_apply_yaw_offset, boss_attach_child_to_mother,
        boss_child_from_index_raw, boss_clear_child_link, boss_count_children,
        boss_dying, boss_find_child_obj, boss_get_mother_obj, boss_keeprel_to_player,
        boss_obj_index_or_null, boss_prune_family_links, bossdelayexplode_strat,
        circdelayexplode_init, circdelayexplode_strat, copy_pos, delayexplode_strat,
        delayremove_strat, frame_tick_mod, hmissile1_remove, hmissile1_remove_strat,
        hmissile1_strat, homingflat_strat, make_exp_obj, make_fol_exp_obj,
        make_large_exp_obj, make_medium_exp_obj, make_small_exp_obj,
        projectile_target_obj, relelaserhome_strat, set_hard_vars,
        strat_aim_3d, strat_aim_yaw, strat_boss_delay_explode_init,
        strat_boss_explode_init, strat_cos, strat_find_near_colltype,
        strat_find_near_shape, strat_fire_relslowlaser, strat_fire_relslowlaserhome,
        strat_move3d, strat_obj_from_ptr, strat_obj_index_or_null, strat_phase_offset,
        strat_pitch_toward, strat_points_positive_z, strat_qboss_explode_init,
        strat_random_centered, strat_relslowelaser_speed, strat_sin, strat_tab_scaled,
        zaco2_istrat,
    };
    /// Alias matching this lane's earlier draft name for
    /// `crate::enemy_a::strat_points_positive_z`.
    pub(crate) use crate::enemy_a::strat_points_positive_z as points_positive_z;

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

    // Collision types (STRATEQU.INC:950-955): acf_colltype* aliases, not
    // 0x02/0x04/0x08 (the old ZENEMY=0x08 aliased the player laser bit).
    pub const COLLTYPE_ENEMY2: u8 = 0x20; // acf_colltype3
    pub const COLLTYPE_ENEMYWEAP: u8 = 0x40; // acf_colltype4
    pub const COLLTYPE_ZENEMY: u8 = 0x01; // acf_colltype6
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

    /// C `SfRtl_Random()` — there is exactly ONE `g_rndval` in C. The
    /// enemy_a helpers this lane calls at runtime (`strat_explode`,
    /// `addrnd2pos_xy`, `strat_random_centered`, ...) advance the PRNG
    /// cell at `ea_wm::RNDVAL` (0x1F00), so this lane draws from the SAME
    /// cell or C-trace parity would desync the moment a shared helper
    /// rolls. (crate::common::sv::RNDVAL is a reported cross-lane
    /// divergence to consolidate; the common-lane projectile path this
    /// lane uses never rolls the PRNG.)
    #[inline]
    pub fn sfrtl_random(g: &mut Game) -> u16 {
        crate::enemy_a::ea_random(g)
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
// Numeric constants for this lane (C strat_enemy.c #defines, verbatim).
// ============================================================

// --- Homing projectile stats (strat_enemy.c:149-151, 296-299) ---
const HMISSILE1_SPEED: u8 = 60;
const HMISSILE1_LIFE: u8 = 100;
const HMISSILE1_AP: u8 = 8;
const HPLASMA_SPEED: u8 = 60;
const HPLASMA_LIFE: u8 = 50;
const HPLASMA_AP: u8 = 10;
const RELSLOWELASERHOME_LIFE: u8 = 40; // strat_enemy.c:192
const RELSLOWELASERHOME_AP: u8 = 2; // strat_enemy.c:193
const EXPSHAPE_LARGE: u16 = 3; // strat_enemy.c:6900

// --- boss7 (strat_enemy.c:274-300) ---
const BOSS7_HP: u8 = 40;
const BOSS7_HATCH_HP: u8 = 16;
const BOSS7_LAUNCHER_HP: u8 = 8;
const BOSS7_SCALE: u32 = 3;
const BOSS7_EXP_FRAMES: u8 = 24;
const BOSS7_SPAWN_SOUND: u8 = 0x5B;
const BOSS7_OPEN_SOUND: u8 = 0x58;
const BOSS7_HATCH_FIRE_DELAY: u16 = 5;
const BOSS7_LAUNCH_FIRE_DELAY: u16 = 5;
const SH_BOSS_7_1: u16 = 56;
const SH_BOSS_7_0: u16 = 240;
const SH_BOSS_7_1O: u16 = 242;
const SH_BOSS_7_2: u16 = 243;
const SH_BOSS_7_3: u16 = 244;
const SH_BOSS_7_4: u16 = 245;
const BOSS7_CHILD_HATCH: u8 = 1;
const BOSS7_CHILD_SHIELD: u8 = 2;
const BOSS7_CHILD_LAUNCHER_T: u8 = 3;
const BOSS7_CHILD_LAUNCHER_B: u8 = 4;
const BOSS7_SFLAG_HATCH: u8 = 0x10;
const BOSS7_SFLAG_LAUNCH: u8 = 0x20;
const BOSS7_SFLAG_SWAY: u8 = BOSS7_SFLAG_HATCH;
const HF2_MASK: u8 = 0x02; // strat_enemy.c:176

// --- bossA (strat_enemy.c:245-273) ---
const BOSSA_AP: u8 = 16;
const BOSSA_TURRET_HP: u8 = 12;
const BOSSA_CUP_HP: u8 = 24;
const BOSSA_SCALE: u32 = 2;
const BOSSA_EXP_FRAMES: u8 = 30;
const BOSSA_CHILD_TURRET_L: u8 = 1;
const BOSSA_CHILD_TURRET_M: u8 = 2;
const BOSSA_CHILD_TURRET_R: u8 = 3;
const BOSSA_CHILD_CUP_L: u8 = 4;
const BOSSA_CHILD_CUP_M: u8 = 5;
const BOSSA_CHILD_CUP_R: u8 = 6;
const BOSSA_PARENT_FLAG_ATTACK_DONE: u8 = 0x10;
const BOSSA_CUP_FLAG_FIRED: u8 = 0x20;
const BOSSA_TURRET_FIRE_DELAY: u16 = 15;
const BOSSA_PARENT_MISSILE_PERIOD: u16 = 64;
const BOSSA_CUP_GO_TIME: u8 = 45;
const BOSSA_CUP_RETURN_TIME: u8 = 30;
const BOSSA_CUP_STATE_COVER: u8 = 0;
const BOSSA_CUP_STATE_UP: u8 = 1;
const BOSSA_CUP_STATE_GO: u8 = 2;
const BOSSA_CUP_STATE_RETURN: u8 = 3;
const BOSSA_CUP_STATE_DOWN: u8 = 4;
const BOSSA_CUP_STATE_IROTATE: u8 = 5;
const BOSSA_CUP_STATE_ROTATE: u8 = 6;
const SH_BOSS_A_1: u16 = 248;
const SH_BOSS_A_2: u16 = 249;
const SH_BOSS_A_6: u16 = 250;

// --- spacepilon (strat_enemy.c GA3STRAT block) ---
const SH_PILON: u16 = 615;
const SPACEPILON_HP: u8 = 6;
const SPACEPILON_AP: u8 = 0;
const SPACEPILON_ZOOMIN_CNT: u8 = 40;
const SPACEPILON_RELEASE_CNT: u8 = 150;
const SPACEPILON_ROT_STEP: u8 = 8;
const SPACEPILON_FIRE_MASK: u16 = 31;
const SPACEPILON_CHILD1_ROT: i8 = 85;
const SPACEPILON_CHILD2_ROT: i8 = -85;
const SPACEPILON_PILON_SBYTE3_INIT: u8 = 10;
const SPACEPILON_PILON_RELPOSY_INIT: i8 = -62; // -500/8
const SPACEPILON_PILON_RELPOSY_TGT: i8 = -12; // -100/8
const SPACEPILON_PILON_EXTEND_STEP: i8 = -10; // -80/8
const SPACEPILON_PILON_RETRACT_STEP: i8 = 10; // 80/8
const SPACEPILON_PILON_ORBIT_SCALE: u32 = 3;
const SPACEPILON_SPAWN_ZOFF: i16 = -500;

// --- bossF (strat_enemy.c GB2STRAT block) ---
const SH_BOSS_F_1: u16 = 251;
const SH_BOSS_F_2: u16 = 252;
const SH_BOSS_F_4: u16 = 120;
const SH_BOSS_F_8: u16 = 253;
const SH_BOSS_F_9: u16 = 254;
const SH_BOSS_F_8A: u16 = 255;
const SH_BOSS_F_9A: u16 = 256;
const BOSSF_SCALE: u32 = 3;
const BOSSF_LAUNCHER_AP: u8 = 10;
const BOSSF_LAUNCHER_HP: u8 = 4;
const BOSSF_LAUNCHER2_HP: u8 = 8;
const BOSSF_SPACE_VIEWCY: i16 = -60;
const BOSSF_CHILD_A: u8 = 1;
const BOSSF_CHILD_B: u8 = 2;
const BOSSF_CHILD_TUR1: u8 = 1;
const BOSSF_CHILD_TUR2: u8 = 2;
const BOSSF_CHILD_TUR3: u8 = 3;
const BOSSF_CHILD_TUR4: u8 = 4;
const BOSSF_CHILD_TUR5: u8 = 5;
const BOSSF_CHILD_TUR6: u8 = 6;
const WEAPON_SCALE: u32 = 2;

// ============================================================
// Private fire helpers — DUPLICATE of enemy_a's `boss1_fire_hmissile1`
// / `boss1_fire_hplasma` (strat_enemy.c:2183/2218). enemy_a keeps them
// private, so this lane carries verbatim copies for bossA. Consolidate
// once the front-half lane exports them.
// ============================================================

/// C `boss1_fire_hmissile1` (strat_enemy.c:2183).
#[allow(clippy::too_many_arguments)]
fn boss1_fire_hmissile1(
    g: &mut Game,
    self_idx: u16,
    target: u16,
    offx: i16,
    offy: i16,
    offz: i16,
    pitch: u8,
    yaw: u8,
) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = strat_make_obj(g, 0)?;
    let me = g.objs.aliens[self_idx as usize];
    boss_apply_yaw_offset(g, shot, &me, offx, offy, offz);
    let s_tick = sid(g, hmissile1_strat);
    let s_coll = sid(g, strat_projectile_on_collide);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.rotz = me.rotz;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_coll);
    al.hp = 2;
    al.ap = HMISSILE1_AP;
    al.vel = HMISSILE1_SPEED;
    al.count = HMISSILE1_LIFE;
    al.snd2 = 2;
    al.type_ = ATMISSILE | ATZREMOVE;
    al.sflags |= ASF_SHADOW;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    strat_gen_vecs_3d(al);
    Some(shot)
}

/// C `boss1_fire_hplasma` (strat_enemy.c:2218).
#[allow(clippy::too_many_arguments)]
fn boss1_fire_hplasma(
    g: &mut Game,
    self_idx: u16,
    target: u16,
    offx: i16,
    offy: i16,
    offz: i16,
    pitch: u8,
    yaw: u8,
) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = strat_make_obj(g, 0)?;
    let me = g.objs.aliens[self_idx as usize];
    boss_apply_yaw_offset(g, shot, &me, offx, offy, offz);
    let s_tick = sid(g, homingflat_strat);
    let s_coll = sid(g, strat_projectile_on_collide);
    let s_exp = sid(g, hmissile1_remove_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = pitch;
    al.roty = yaw;
    al.rotz = me.rotz;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = 1;
    al.ap = HPLASMA_AP;
    al.vel = HPLASMA_SPEED;
    al.count = HPLASMA_LIFE;
    al.snd2 = 6;
    al.type_ = ATLASER | ATZREMOVE;
    al.sflags |= ASF_INVISIBLE;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    al.sbyte1 = yaw;
    al.sbyte2 = pitch;
    al.rotx = 0;
    al.roty = DEG180;
    Some(shot)
}

// ============================================================
// EB_PART_1_BEGIN (strat_enemy.c:1297-1974 — boss7 phase machine)
// ============================================================

/// C `boss7_part_explode` (strat_enemy.c:1297).
fn boss7_part_explode(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].flags |= AFEXP;
    play_se(g, 0x10);
    g.objs.aldead = 1;
}

/// C `boss7_fire_hplasma` (strat_enemy.c:1113).
fn boss7_fire_hplasma(g: &mut Game, self_idx: u16, target: u16) -> Option<u16> {
    if !g.objs.aliens[target as usize].active {
        return None;
    }
    let shot = strat_make_obj(g, 0)?;
    let me = g.objs.aliens[self_idx as usize];
    boss_apply_yaw_offset(g, shot, &me, 0, 0, (40i16 << BOSS7_SCALE) >> 2);
    let s_tick = sid(g, homingflat_strat);
    let s_coll = sid(g, strat_projectile_on_collide);
    let s_exp = sid(g, hmissile1_remove_strat);
    let al = &mut g.objs.aliens[shot as usize];
    al.rotx = me.rotx;
    al.roty = me.roty;
    al.rotz = me.rotz;
    al.stratptr = Some(s_tick);
    al.collstratptr = Some(s_coll);
    al.expstratptr = Some(s_exp);
    al.hp = 1;
    al.ap = HPLASMA_AP;
    al.vel = HPLASMA_SPEED;
    al.count = HPLASMA_LIFE;
    al.snd2 = 6;
    al.type_ = ATLASER | ATZREMOVE;
    al.sflags |= ASF_INVISIBLE;
    al.collflags = ACF_FIRSTFRAME | ACF_WEAPON | ACF_COLLTYPE4;
    al.immuneptr = strat_obj_index_or_null(self_idx);
    al.fireobjptr = target + 1;
    al.sbyte1 = al.roty;
    al.sbyte2 = al.rotx;
    al.rotx = 0;
    al.roty = DEG180;
    Some(shot)
}

/// C `boss7hatch_strat` (strat_enemy.c:1306).
fn boss7hatch_strat(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_INVISIBLE;
        al.sflags |= ASF_NOHITAFFECT;
    }

    if g.objs.aliens[mother_idx as usize].sflags2 & BOSS7_SFLAG_HATCH != 0 {
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        let animframe = g.objs.aliens[idx as usize].animframe;
        if animframe < 8 {
            if animframe == 0 {
                play_se(g, 0x5A);
            }
            g.objs.aliens[idx as usize].animframe += 1;
        } else {
            let al = &mut g.objs.aliens[idx as usize];
            al.colframe = al.colframe.wrapping_add(1) & 3;
            if g.vars.gameframe % BOSS7_HATCH_FIRE_DELAY == 0 {
                if let Some(child) = boss7hatchfire_srou(g, idx) {
                    let c = &mut g.objs.aliens[child as usize];
                    c.roty = c.roty.wrapping_sub(7);
                }
                if let Some(child) = boss7hatchfire_srou(g, idx) {
                    let c = &mut g.objs.aliens[child as usize];
                    c.roty = c.roty.wrapping_add(21);
                }
            }
        }
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = 4;
        if al.animframe == 8 {
            play_se(g, 0x59);
        }
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe > 0 {
            al.animframe -= 1;
        }
    }

    let mother = g.objs.aliens[mother_idx as usize];
    boss_apply_yaw_offset(g, idx, &mother, -(20i16 << BOSS7_SCALE), 0, 0);
}

/// C `boss7hatch_coll` (strat_enemy.c:1355).
fn boss7hatch_coll(g: &mut Game, idx: u16) {
    let mother = boss_get_mother_obj(g, idx);
    let hatch_open =
        mother.map_or(false, |m| g.objs.aliens[m as usize].sflags2 & BOSS7_SFLAG_HATCH != 0);
    if mother.is_none() || !hatch_open {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags = 0;
        al.sflags &= !ASF_COLLIDE;
        return;
    }
    strat_hit_flash(g, idx);
}

/// C `boss7hatch_init` (strat_enemy.c:1365).
fn boss7hatch_init(g: &mut Game, idx: u16) {
    let mut hp = BOSS7_HATCH_HP;
    if currentlevel(g) == 2 {
        hp = BOSS7_HATCH_HP.wrapping_mul(2);
    }
    let s_strat = sid(g, boss7hatch_strat);
    let s_coll = sid(g, boss7hatch_coll);
    let s_exp = sid(g, boss7_part_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = hp;
        al.ap = 10;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_SHADOW | ASF_INVISIBLE;
        al.depthoffset = 1;
        al.colframe = 0;
        al.animframe = 0;
    }
    set_bossmaxhp(g, bossmaxhp(g).wrapping_add(hp as u16));
}

/// C `boss7launcher_common_strat` (strat_enemy.c:1388).
fn boss7launcher_common_strat(g: &mut Game, idx: u16, yoff: i16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_INVISIBLE;
        al.sflags |= ASF_NOHITAFFECT;
    }

    if g.objs.aliens[mother_idx as usize].sflags2 & BOSS7_SFLAG_LAUNCH != 0 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags &= !ASF_NOHITAFFECT;
            al.colframe = al.colframe.wrapping_add(1) & 3;
        }
        let animframe = g.objs.aliens[idx as usize].animframe;
        if animframe < 9 {
            if animframe == 0 {
                play_se(g, 0x5A);
            }
            g.objs.aliens[idx as usize].animframe += 1;
        } else if g.vars.gameframe % BOSS7_LAUNCH_FIRE_DELAY == 0 {
            if let Some(p) = player_idx(g) {
                let _ = boss7launcher_fire_hmissile1(g, idx, p);
            }
        }
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = 4;
        if al.animframe == 9 {
            play_se(g, 0x59);
        }
        let al = &mut g.objs.aliens[idx as usize];
        if al.animframe > 0 {
            al.animframe -= 1;
        }
    }

    let mother = g.objs.aliens[mother_idx as usize];
    boss_apply_yaw_offset(g, idx, &mother, 30i16 << BOSS7_SCALE, yoff, 0);
}

/// C `boss7launcherT_strat` (strat_enemy.c:1432).
fn boss7launcherT_strat(g: &mut Game, idx: u16) {
    boss7launcher_common_strat(g, idx, -(15i16 << BOSS7_SCALE));
}

/// C `boss7launcherB_strat` (strat_enemy.c:1436).
fn boss7launcherB_strat(g: &mut Game, idx: u16) {
    boss7launcher_common_strat(g, idx, 25i16 << BOSS7_SCALE);
}

/// C `boss7launcher_coll` (strat_enemy.c:1440).
fn boss7launcher_coll(g: &mut Game, idx: u16) {
    let mother = boss_get_mother_obj(g, idx);
    let launch =
        mother.map_or(false, |m| g.objs.aliens[m as usize].sflags2 & BOSS7_SFLAG_LAUNCH != 0);
    let hf = g.objs.aliens[idx as usize].hitflags;
    if mother.is_none() || !launch || (hf & HF2_MASK) == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags = 0;
        al.sflags &= !ASF_COLLIDE;
        return;
    }
    g.objs.aliens[idx as usize].hitflags = 0;
    strat_hit_flash(g, idx);
}

/// C `boss7launcher_init_common` (strat_enemy.c:1453).
fn boss7launcher_init_common(g: &mut Game, idx: u16, strat: StrategyFn) {
    let mut hp = BOSS7_LAUNCHER_HP;
    if currentlevel(g) == 2 {
        hp = BOSS7_LAUNCHER_HP.wrapping_mul(2);
    }
    let s_strat = sid(g, strat);
    let s_coll = sid(g, boss7launcher_coll);
    let s_exp = sid(g, boss7_part_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = hp;
        al.ap = 10;
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_SHADOW | ASF_INVISIBLE;
        al.depthoffset = 1;
        al.colframe = 0;
        al.animframe = 0;
    }
    set_bossmaxhp(g, bossmaxhp(g).wrapping_add(hp as u16));
}

/// C `boss7launcherT_init` (strat_enemy.c:1476).
fn boss7launcherT_init(g: &mut Game, idx: u16) {
    boss7launcher_init_common(g, idx, boss7launcherT_strat);
}

/// C `boss7launcherB_init` (strat_enemy.c:1480).
fn boss7launcherB_init(g: &mut Game, idx: u16) {
    boss7launcher_init_common(g, idx, boss7launcherB_strat);
}

/// C `boss7shield_strat` (strat_enemy.c:1484).
fn boss7shield_strat(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    g.objs.aliens[idx as usize].sflags &= !ASF_INVISIBLE;
    let mother = g.objs.aliens[mother_idx as usize];
    boss_apply_yaw_offset(g, idx, &mother, 20i16 << BOSS7_SCALE, 0, 0);
}

/// C `boss7shield_init` (strat_enemy.c:1501).
fn boss7shield_init(g: &mut Game, idx: u16) {
    let s_strat = sid(g, boss7shield_strat);
    let s_exp = sid(g, boss7_part_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s_strat);
    al.collstratptr = None;
    al.expstratptr = Some(s_exp);
    al.hp = HARD_HP;
    al.ap = 10;
    al.collflags |= COLLTYPE_ENEMY1;
    al.sflags |= ASF_SHADOW | ASF_INVISIBLE;
}

/// C `boss7_spawn_child` (strat_enemy.c:1514).
fn boss7_spawn_child(g: &mut Game, mother: u16, child_num: u8, init_fn: StrategyFn) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    let shape = match child_num {
        BOSS7_CHILD_HATCH => SH_BOSS_7_0,
        BOSS7_CHILD_SHIELD => SH_BOSS_7_2,
        BOSS7_CHILD_LAUNCHER_T => SH_BOSS_7_3,
        BOSS7_CHILD_LAUNCHER_B => SH_BOSS_7_4,
        _ => SH_BOSS_7_1,
    };
    g.objs.aliens[child as usize].shape = shape;
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        obj_free(g, child);
        return None;
    }
    init_fn(g, child);
    Some(child)
}

/// C `boss7_enter_alldead` (strat_enemy.c:1539).
fn boss7_enter_alldead(g: &mut Game, idx: u16) {
    let s_strat = sid(g, boss7alldead_strat);
    let s_coll = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.shape = SH_BOSS_7_1;
        al.sflags &= !ASF_NOHITAFFECT;
        al.sflags2 &= !(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    }
    play_se(g, BOSS7_OPEN_SOUND);
}

/// C `boss7_parent_cont` (strat_enemy.c:1552).
fn boss7_parent_cont(g: &mut Game, idx: u16) {
    boss_keeprel_to_player(g, idx);
    let vel = g.objs.aliens[idx as usize].vel;
    strat_move3d(g, idx, vel, 0);

    if boss_find_child_obj(g, idx, BOSS7_CHILD_LAUNCHER_T).is_none()
        && boss_find_child_obj(g, idx, BOSS7_CHILD_LAUNCHER_B).is_none()
    {
        if let Some(shield) = boss_find_child_obj(g, idx, BOSS7_CHILD_SHIELD) {
            obj_free(g, shield);
        }
    }

    if boss_count_children(g, idx) == 0 {
        boss7_enter_alldead(g, idx);
        return;
    }

    add_player_z(g, idx);
}

/// C `boss7a_strat` (strat_enemy.c:1578).
fn boss7a_strat(g: &mut Game, idx: u16) {
    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        if (me.worldz as i32 - pl.worldz as i32).abs() >= (170i32 << BOSS7_SCALE) {
            if g.vars.gameframe & 1 == 0 {
                strat_speed_to(&mut g.objs.aliens[idx as usize], 0, 1);
            }
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.roty != DEG180 {
                    al.colframe = al.colframe.wrapping_add(1) & 3;
                    al.roty = al.roty.wrapping_add(1);
                    if al.roty == DEG90 {
                        al.shape = SH_BOSS_7_1O;
                    }
                }
            }
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.worldy < -(40i16 << BOSS7_SCALE) {
                    boss7b_init(g, idx);
                    return;
                }
                al.worldy = al.worldy.wrapping_add(2);
            }
        }
    }
    boss7_parent_cont(g, idx);
}

/// C `boss7b_init` (strat_enemy.c:1608).
fn boss7b_init(g: &mut Game, idx: u16) {
    if boss_find_child_obj(g, idx, BOSS7_CHILD_HATCH).is_none() {
        boss7c2_init(g, idx);
        return;
    }
    let s = sid(g, boss7b_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 |= BOSS7_SFLAG_HATCH;
    al.sflags2 &= !BOSS7_SFLAG_LAUNCH;
    al.sbyte4 = 30;
    al.stratptr = Some(s);
}

/// C `boss7b_strat` (strat_enemy.c:1622).
fn boss7b_strat(g: &mut Game, idx: u16) {
    if !achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180 - DEG22, 4) {
        boss7_parent_cont(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte4 > 0 {
            al.sbyte4 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte4 == 0 {
        boss7d_init(g, idx);
        return;
    }
    boss7_parent_cont(g, idx);
}

/// C `boss7launch_cont` (strat_enemy.c:1640).
fn boss7launch_cont(g: &mut Game, idx: u16) {
    if boss_find_child_obj(g, idx, BOSS7_CHILD_HATCH).is_none()
        || boss_find_child_obj(g, idx, BOSS7_CHILD_LAUNCHER_B).is_none()
    {
        boss7_parent_cont(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.worldy >= -(30i16 << BOSS7_SCALE) {
            al.worldy = al.worldy.wrapping_add(2);
        }
    }
    boss7_parent_cont(g, idx);
}

/// C `boss7c_init` (strat_enemy.c:1652).
fn boss7c_init(g: &mut Game, idx: u16) {
    if boss_find_child_obj(g, idx, BOSS7_CHILD_SHIELD).is_none() {
        boss7e_init(g, idx);
        return;
    }
    let s = sid(g, boss7c_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 &= !BOSS7_SFLAG_HATCH;
    al.sflags2 |= BOSS7_SFLAG_LAUNCH;
    al.sbyte4 = 50;
    al.stratptr = Some(s);
}

/// C `boss7c_strat` (strat_enemy.c:1666).
fn boss7c_strat(g: &mut Game, idx: u16) {
    if !achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180 + DEG22, 4) {
        boss7launch_cont(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte4 > 0 {
            al.sbyte4 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte4 == 0 {
        boss7e_init(g, idx);
        return;
    }
    boss7launch_cont(g, idx);
}

/// C `boss7c2_init` (strat_enemy.c:1684).
fn boss7c2_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss7c2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 &= !BOSS7_SFLAG_LAUNCH;
    al.sbyte4 = 15;
    al.stratptr = Some(s);
}

/// C `boss7c2_strat` (strat_enemy.c:1693).
fn boss7c2_strat(g: &mut Game, idx: u16) {
    if !achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180 + DEG45 + DEG22, 4) {
        boss7launch_cont(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte4 > 0 {
            al.sbyte4 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte4 == 0 {
        boss7c_init(g, idx);
        return;
    }
    boss7launch_cont(g, idx);
}

/// C `boss7d_init` (strat_enemy.c:1711).
fn boss7d_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss7d_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 &= !(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    al.sbyte3 = 0;
    al.stratptr = Some(s);
}

/// C `boss7d_strat` (strat_enemy.c:1720).
fn boss7d_strat(g: &mut Game, idx: u16) {
    {
        let sb3 = g.objs.aliens[idx as usize].sbyte3;
        let dy = (strat_sin(sb3) * 8.0).round() as i16;
        let dz = (strat_cos(sb3) * 2.0).round() as i16;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(dy);
        al.worldz = al.worldz.wrapping_sub(dz);
    }
    achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 4);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte3 = al.sbyte3.wrapping_add(4);
    }
    if g.objs.aliens[idx as usize].sbyte3 == 192 {
        boss7c_init(g, idx);
        return;
    }
    boss7_parent_cont(g, idx);
}

/// C `boss7e_init` (strat_enemy.c:1735).
fn boss7e_init(g: &mut Game, idx: u16) {
    if boss_find_child_obj(g, idx, BOSS7_CHILD_HATCH).is_none() {
        boss7c2_init(g, idx);
        return;
    }
    let s = sid(g, boss7e_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags2 &= !(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    al.sbyte3 = 192;
    al.stratptr = Some(s);
}

/// C `boss7e_strat` (strat_enemy.c:1748).
fn boss7e_strat(g: &mut Game, idx: u16) {
    {
        let sb3 = g.objs.aliens[idx as usize].sbyte3;
        let dy = (strat_sin(sb3) * 8.0).round() as i16;
        let dz = (strat_cos(sb3) * 2.0).round() as i16;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_sub(dy);
        al.worldz = al.worldz.wrapping_sub(dz);
    }
    achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 4);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte3 = al.sbyte3.wrapping_add(4);
    }
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        boss7b_init(g, idx);
        return;
    }
    boss7_parent_cont(g, idx);
}

/// C `boss7_alldead_cont` (strat_enemy.c:1763).
fn boss7_alldead_cont(g: &mut Game, idx: u16) {
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.colframe = al.colframe.wrapping_add(1) & 3;
    }
    add_player_z(g, idx);
}

/// C `boss7alldead_strat` (strat_enemy.c:1774).
fn boss7alldead_strat(g: &mut Game, idx: u16) {
    achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 3);
    {
        let px = g.vars.player_posx;
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = strat_chase_proportional(al.worldx, px, 4);
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        if (me.worldz as i32 - pl.worldz as i32).abs() < 600 {
            boss7alldeada_init(g, idx);
            boss7alldeada_strat(g, idx);
            return;
        }
        strat_aim_yaw(g, idx, &pl, 2);
    }
    strat_speed_to(&mut g.objs.aliens[idx as usize], 30, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.worldy >= -(40i16 << BOSS7_SCALE) {
            al.worldy = al.worldy.wrapping_add(10);
        }
    }

    let frame_masked = (g.vars.gameframe & 31) as u8;
    if (frame_masked == 25 || frame_masked == 30) && player_idx(g).is_some() {
        let p = player_idx(g).unwrap();
        let _ = boss7_fire_hplasma(g, idx, p);
    }

    boss7_alldead_cont(g, idx);
}

/// C `boss7alldeada_init` (strat_enemy.c:1808).
fn boss7alldeada_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss7alldeada_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = (DEG180 + DEG22) / 4;
    al.sflags2 ^= BOSS7_SFLAG_SWAY;
}

/// C `boss7alldeada_strat` (strat_enemy.c:1817).
fn boss7alldeada_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sflags2 & BOSS7_SFLAG_SWAY != 0 {
            al.rotz = al.rotz.wrapping_sub(1);
            al.roty = al.roty.wrapping_sub(4);
        } else {
            al.rotz = al.rotz.wrapping_add(1);
            al.roty = al.roty.wrapping_add(4);
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        boss7alldeadc_init(g, idx);
        boss7alldeadc_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    boss7_alldead_cont(g, idx);
}

/// C `boss7alldeadc_init` (strat_enemy.c:1839).
fn boss7alldeadc_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss7alldeadc_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = 50;
}

/// C `boss7alldeadc_strat` (strat_enemy.c:1847).
fn boss7alldeadc_strat(g: &mut Game, idx: u16) {
    strat_speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
    achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 3);
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        boss7alldeadb_init(g, idx);
        boss7alldeadb_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    boss7_alldead_cont(g, idx);
}

/// C `boss7alldeadb_init` (strat_enemy.c:1864).
fn boss7alldeadb_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss7alldeadb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = (DEG180 + DEG22) / 4;
}

/// C `boss7alldeadb_strat` (strat_enemy.c:1872).
fn boss7alldeadb_strat(g: &mut Game, idx: u16) {
    strat_speed_to(&mut g.objs.aliens[idx as usize], 30, 1);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sflags2 & BOSS7_SFLAG_SWAY != 0 {
            al.rotz = al.rotz.wrapping_add(1);
            al.roty = al.roty.wrapping_add(4);
        } else {
            al.rotz = al.rotz.wrapping_sub(1);
            al.roty = al.roty.wrapping_sub(4);
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        boss7_enter_alldead(g, idx);
        boss7alldead_strat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;
    boss7_alldead_cont(g, idx);
}

/// C `boss7exp_init` (strat_enemy.c:1895).
fn boss7exp_init(g: &mut Game, idx: u16) {
    let s = sid(g, boss7exp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
        al.flags |= AFEXP;
        al.count = BOSS7_EXP_FRAMES;
        al.vx = 0;
        al.vy = -30;
        al.vz = 20;
        al.sbyte2 = 2;
        al.sbyte3 = 0;
    }
    set_bossflags(g, bossflags(g) | BF_DYING);
    play_se(g, 0x10);
}

/// C `boss7exp_strat` (strat_enemy.c:1915).
fn boss7exp_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(al.sbyte3);
        if al.sbyte3 < 8 && g.vars.gameframe & 3 == 0 {
            al.sbyte3 += 1;
        }
        al.vy = al.vy.wrapping_add(1);
    }
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);

    if !strat_count_down(&mut g.objs.aliens[idx as usize]) {
        return;
    }

    g.vars.gameflags |= GF_BOSSDEAD;
    set_bossflags(g, bossflags(g) & !BF_DYING);
    set_bossmaxhp(g, 0);
    g.objs.aldead = 1;
}

/// C `Strat_Boss7_Init` (strat_enemy.c:1957).
pub fn strat_boss7_init(g: &mut Game, idx: u16) {
    let mut hp = BOSS7_HP;
    if currentlevel(g) == 2 {
        hp = BOSS7_HP.wrapping_mul(2);
    }
    let s_strat = sid(g, boss7a_strat);
    let s_exp = sid(g, boss7exp_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = None;
        al.expstratptr = Some(s_exp);
        al.collflags |= COLLTYPE_ENEMY1;
        al.sflags |= ASF_SHADOW | ASF_NOHITAFFECT;
        al.depthoffset = 1;
        al.hp = hp;
        al.ap = 20;
        al.vel = 20;
        al.sflags2 &= !(BOSS7_SFLAG_HATCH | BOSS7_SFLAG_LAUNCH);
    }
    g.vars.gameflags &= !GF_BOSSDEAD;
    set_bossflags(g, bossflags(g) & !BF_DYING);
    set_bossmaxhp(g, hp as u16);
    g.vars.meters = 1;

    let _ = boss7_spawn_child(g, idx, BOSS7_CHILD_HATCH, boss7hatch_init);
    let _ = boss7_spawn_child(g, idx, BOSS7_CHILD_SHIELD, boss7shield_init);
    let _ = boss7_spawn_child(g, idx, BOSS7_CHILD_LAUNCHER_T, boss7launcherT_init);
    let _ = boss7_spawn_child(g, idx, BOSS7_CHILD_LAUNCHER_B, boss7launcherB_init);

    play_se(g, BOSS7_SPAWN_SOUND);
}
// EB_PART_1_END

// ============================================================
// EB_PART_2_BEGIN (strat_enemy.c:2788-3542 — bossA)
// ============================================================

/// C `bossA_get_turret_offset` (strat_enemy.c:2797).
fn bossa_get_turret_offset(child_num: u8) -> Option<(i16, i16)> {
    match child_num {
        BOSSA_CHILD_TURRET_L => Some((-85i16 << BOSSA_SCALE, -50i16 << BOSSA_SCALE)),
        BOSSA_CHILD_TURRET_M => Some((0, -40i16 << BOSSA_SCALE)),
        BOSSA_CHILD_TURRET_R => Some((85i16 << BOSSA_SCALE, -50i16 << BOSSA_SCALE)),
        _ => None,
    }
}

/// C `bossA_spawn_child` (strat_enemy.c:2820).
fn bossa_spawn_child(g: &mut Game, mother: u16, child_num: u8, init_fn: StrategyFn) -> Option<u16> {
    let shape = if child_num <= BOSSA_CHILD_TURRET_R {
        SH_BOSS_A_1
    } else {
        SH_BOSS_A_6
    };
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    g.objs.aliens[child as usize].shape = shape;
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        obj_free(g, child);
        return None;
    }
    init_fn(g, child);
    Some(child)
}

/// C `bossA_release_children` (strat_enemy.c:2845).
fn bossa_release_children(g: &mut Game, idx: u16) {
    boss_prune_family_links(g, idx);
    let mut cur = g.objs.aliens[idx as usize].sword1 as u16;
    let mut guard = NUMBER_AL as i32 + 1;
    while cur != 0 && guard > 0 {
        guard -= 1;
        let Some(child) = boss_child_from_index_raw(cur) else {
            break;
        };
        let next = g.objs.aliens[child as usize].sword1 as u16;
        boss_clear_child_link(g, child);
        obj_free(g, child);
        cur = next;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.sword1 = 0;
    al.sflags3 &= !ASF3_MOTHEROBJ;
}

/// C `bossA_live_turret_count` (strat_enemy.c:2874).
fn bossa_live_turret_count(g: &mut Game, idx: u16) -> u8 {
    let mut count = 0;
    for child_num in BOSSA_CHILD_TURRET_L..=BOSSA_CHILD_TURRET_R {
        if boss_find_child_obj(g, idx, child_num).is_some() {
            count += 1;
        }
    }
    count
}

/// C `bossA_live_cup_count` (strat_enemy.c:2891).
fn bossa_live_cup_count(g: &mut Game, idx: u16) -> u8 {
    let mut count = 0;
    for child_num in BOSSA_CHILD_CUP_L..=BOSSA_CHILD_CUP_R {
        if boss_find_child_obj(g, idx, child_num).is_some() {
            count += 1;
        }
    }
    count
}

/// C `bossA_linked_turret` (strat_enemy.c:2908).
fn bossa_linked_turret(g: &mut Game, cup: u16) -> Option<u16> {
    let mother = boss_get_mother_obj(g, cup)?;
    let sb2 = g.objs.aliens[cup as usize].sbyte2;
    if sb2 < BOSSA_CHILD_TURRET_L || sb2 > BOSSA_CHILD_TURRET_R {
        return None;
    }
    boss_find_child_obj(g, mother, sb2)
}

/// C `bossA_update_turret_position` (strat_enemy.c:2922).
fn bossa_update_turret_position(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    let Some((offx, offy)) = bossa_get_turret_offset(sb1) else {
        return;
    };
    let mother = g.objs.aliens[mother_idx as usize];
    boss_apply_yaw_offset(g, idx, &mother, offx, offy, 0);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = 0;
    al.rotz = mother.rotz;
}

/// C `bossA_cup_set_home` (strat_enemy.c:2941).
fn bossa_cup_set_home(g: &mut Game, idx: u16, lift: i16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let turret = bossa_linked_turret(g, idx);
    if let Some(t) = turret.filter(|&t| g.objs.aliens[t as usize].active) {
        let ta = g.objs.aliens[t as usize];
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = ta.worldx;
        al.worldy = ta.worldy.wrapping_sub(lift);
        al.worldz = ta.worldz;
    } else {
        let sb2 = g.objs.aliens[idx as usize].sbyte2;
        if let Some((offx, offy)) = bossa_get_turret_offset(sb2) {
            let mother = g.objs.aliens[mother_idx as usize];
            boss_apply_yaw_offset(g, idx, &mother, offx, offy.wrapping_sub(lift), 0);
        }
    }
    let mroty = g.objs.aliens[mother_idx as usize].roty;
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = DEG90.wrapping_neg();
    al.roty = DEG180;
    al.rotz = mroty;
}

/// C `bossA_cup_chase_home` (strat_enemy.c:2967).
fn bossa_cup_chase_home(g: &mut Game, idx: u16, lift: i16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let turret = bossa_linked_turret(g, idx);
    if let Some(t) = turret.filter(|&t| g.objs.aliens[t as usize].active) {
        let ta = g.objs.aliens[t as usize];
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = strat_chase_proportional(al.worldx, ta.worldx, 2);
        al.worldy = strat_chase_proportional(al.worldy, ta.worldy.wrapping_sub(lift), 3);
        al.worldz = strat_chase_proportional(al.worldz, ta.worldz, 2);
        return;
    }
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if let Some((offx, offy)) = bossa_get_turret_offset(sb2) {
        let mother = g.objs.aliens[mother_idx as usize];
        boss_apply_yaw_offset(g, idx, &mother, offx, offy.wrapping_sub(lift), 0);
    }
}

/// C `bossA_pick_live_cup` (strat_enemy.c:2993).
fn bossa_pick_live_cup(g: &mut Game, idx: u16) -> Option<u16> {
    let mut cups: [u16; 3] = [0; 3];
    let mut count = 0usize;
    for child_num in BOSSA_CHILD_CUP_L..=BOSSA_CHILD_CUP_R {
        if let Some(child) = boss_find_child_obj(g, idx, child_num) {
            cups[count] = child;
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    let r = sfrtl_random(g) as usize % count;
    Some(cups[r])
}

/// C `bossA_set_all_cup_state` (strat_enemy.c:3016).
fn bossa_set_all_cup_state(g: &mut Game, idx: u16, state: u8) {
    for child_num in BOSSA_CHILD_CUP_L..=BOSSA_CHILD_CUP_R {
        let Some(child) = boss_find_child_obj(g, idx, child_num) else {
            continue;
        };
        let al = &mut g.objs.aliens[child as usize];
        al.stratstate = state;
        if state == BOSSA_CUP_STATE_GO || state == BOSSA_CUP_STATE_IROTATE {
            al.sflags2 &= !BOSSA_CUP_FLAG_FIRED;
            al.count = BOSSA_CUP_GO_TIME;
        } else if state == BOSSA_CUP_STATE_RETURN {
            al.count = BOSSA_CUP_RETURN_TIME;
        }
    }
}

/// C `bossA_update_turret_targets` (strat_enemy.c:3038).
fn bossa_update_turret_targets(g: &mut Game, idx: u16) {
    const TARGETS: [[u8; 3]; 3] = [[0, 0, DEG180], [0, DEG180, 0], [DEG180, 0, 0]];
    let pattern = (g.objs.aliens[idx as usize].sbyte3 % 3) as usize;
    for child_num in BOSSA_CHILD_TURRET_L..=BOSSA_CHILD_TURRET_R {
        let Some(child) = boss_find_child_obj(g, idx, child_num) else {
            continue;
        };
        g.objs.aliens[child as usize].sbyte3 =
            TARGETS[pattern][(child_num - BOSSA_CHILD_TURRET_L) as usize];
    }
}

/// C `bossA_part_coll` (strat_enemy.c:3061).
fn bossa_part_coll(g: &mut Game, idx: u16) {
    let sflags = g.objs.aliens[idx as usize].sflags;
    if sflags & (ASF_INVISIBLE | ASF_NOHITAFFECT) != 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.hitflags = 0;
        al.sflags &= !ASF_COLLIDE;
        return;
    }
    strat_hit_flash(g, idx);
}

/// C `bossA_turret_exp_init` (strat_enemy.c:3072).
fn bossa_turret_exp_init(g: &mut Game, idx: u16) {
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        g.objs.aliens[mother as usize].sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
    }
    play_se(g, 0x10);
    g.objs.aldead = 1;
}

/// C `bossA_cup_exp_init` (strat_enemy.c:3082).
fn bossa_cup_exp_init(g: &mut Game, idx: u16) {
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        g.objs.aliens[mother as usize].sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
    }
    play_se(g, 0x10);
    g.objs.aldead = 1;
}

/// C `bossA_turret_init_common` (strat_enemy.c:3092).
fn bossa_turret_init_common(g: &mut Game, idx: u16, initial_target: u8) {
    let s_coll = sid(g, bossa_part_coll);
    let s_exp = sid(g, bossa_turret_exp_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = None;
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = BOSSA_TURRET_HP;
        al.ap = BOSSA_AP;
        al.sbyte2 = 60;
        al.sbyte3 = initial_target;
        al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
        al.sflags |= ASF_SHADOW | ASF_NOHITAFFECT;
        al.type_ &= !ATZREMOVE;
    }
    bossa_update_turret_position(g, idx);
}

/// C `bossA_turretL_strat` (strat_enemy.c:3110). Also M/R share this body.
fn bossa_turretL_strat(g: &mut Game, idx: u16) {
    bossa_update_turret_position(g, idx);
    let me = g.objs.aliens[idx as usize];
    if me.sflags & ASF_NOHITAFFECT == 0
        && strat_points_positive_z(&me)
        && g.vars.gameframe % BOSSA_TURRET_FIRE_DELAY == 0
    {
        if let Some(pl) = player(g) {
            let pitch = strat_pitch_toward(&me, &pl);
            let yaw = strat_angle_xz(&me, &pl);
            let p = player_idx(g).unwrap();
            let _ = boss1_fire_hplasma(g, idx, p, 0, -(10i16 << BOSSA_SCALE), 0, pitch, yaw);
        }
    }
    let sb3 = g.objs.aliens[idx as usize].sbyte3;
    achase_angle(&mut g.objs.aliens[idx as usize].roty, sb3, 3);
    add_player_z(g, idx);
}

/// C `bossA_turretM_strat` (strat_enemy.c:3132).
fn bossa_turretM_strat(g: &mut Game, idx: u16) {
    bossa_turretL_strat(g, idx);
}

/// C `bossA_turretR_strat` (strat_enemy.c:3136).
fn bossa_turretR_strat(g: &mut Game, idx: u16) {
    bossa_turretL_strat(g, idx);
}

/// C `bossA_turretL_init` (strat_enemy.c:3140).
fn bossa_turretL_init(g: &mut Game, idx: u16) {
    bossa_turret_init_common(g, idx, 0);
    let s = sid(g, bossa_turretL_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossA_turretM_init` (strat_enemy.c:3145).
fn bossa_turretM_init(g: &mut Game, idx: u16) {
    bossa_turret_init_common(g, idx, DEG180);
    let s = sid(g, bossa_turretM_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossA_turretR_init` (strat_enemy.c:3150).
fn bossa_turretR_init(g: &mut Game, idx: u16) {
    bossa_turret_init_common(g, idx, 0);
    let s = sid(g, bossa_turretR_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossA_cup_init_common` (strat_enemy.c:3155).
fn bossa_cup_init_common(g: &mut Game, idx: u16, turret_child_num: u8) {
    let s_coll = sid(g, bossa_part_coll);
    let s_exp = sid(g, bossa_cup_exp_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = None;
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.hp = BOSSA_CUP_HP;
        al.ap = BOSSA_AP;
        al.sbyte2 = turret_child_num;
        al.stratstate = BOSSA_CUP_STATE_COVER;
        al.collflags |= COLLTYPE_ENEMY1 | COLLTYPE_ENEMYWEAP;
        al.sflags |= ASF_SHADOW | ASF_NOHITAFFECT;
        al.type_ &= !ATZREMOVE;
        al.animframe = 0;
    }
    bossa_cup_set_home(g, idx, 15i16 << BOSSA_SCALE);
}

/// C `bossA_cup_strat` (strat_enemy.c:3174).
fn bossa_cup_strat(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    g.objs.aliens[idx as usize].sflags &= !ASF_INVISIBLE;

    let mut state = g.objs.aliens[idx as usize].stratstate;
    if state == BOSSA_CUP_STATE_IROTATE {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratstate = BOSSA_CUP_STATE_ROTATE;
        al.sword2 = 40;
        al.sflags &= !ASF_NOHITAFFECT;
        state = BOSSA_CUP_STATE_ROTATE;
    }

    match state {
        BOSSA_CUP_STATE_COVER => {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags |= ASF_NOHITAFFECT;
            if al.animframe > 0 && g.vars.gameframe & 1 == 0 {
                al.animframe -= 1;
            }
            bossa_cup_set_home(g, idx, 15i16 << BOSSA_SCALE);
        }
        BOSSA_CUP_STATE_UP => {
            let turret = bossa_linked_turret(g, idx);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags |= ASF_NOHITAFFECT;
                if al.animframe < 7 && g.vars.gameframe & 1 == 0 {
                    al.animframe += 1;
                }
            }
            bossa_cup_chase_home(g, idx, 110i16 << BOSSA_SCALE);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotz = al.rotz.wrapping_add(20);
            }
            achase_angle(&mut g.objs.aliens[idx as usize].rotx, DEG90.wrapping_neg(), 3);
            if let Some(t) = turret {
                g.objs.aliens[t as usize].sflags &= !ASF_NOHITAFFECT;
            }
        }
        BOSSA_CUP_STATE_ROTATE => {
            bossa_cup_chase_home(g, idx, 70i16 << BOSSA_SCALE);
            achase_angle(&mut g.objs.aliens[idx as usize].rotx, 0, 3);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotz = al.rotz.wrapping_add(28);
                if al.sword2 > 0 {
                    al.sword2 -= 1;
                }
            }
            if g.objs.aliens[idx as usize].sword2 == 0 {
                g.objs.aliens[mother_idx as usize].sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
                let al = &mut g.objs.aliens[idx as usize];
                al.stratstate = BOSSA_CUP_STATE_UP;
                al.sflags |= ASF_NOHITAFFECT;
            }
        }
        BOSSA_CUP_STATE_GO => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sflags2 & BOSSA_CUP_FLAG_FIRED == 0 {
                    play_se(g, 0x66);
                    let al = &mut g.objs.aliens[idx as usize];
                    al.sflags2 |= BOSSA_CUP_FLAG_FIRED;
                    al.sflags &= !ASF_NOHITAFFECT;
                }
            }
            if let Some(pl) = player(g) {
                strat_aim_3d(g, idx, &pl, 3);
                let me = g.objs.aliens[idx as usize];
                if me.worldz >= pl.worldz
                    || (me.worldz as i32 - pl.worldz as i32).abs() < 200
                {
                    let al = &mut g.objs.aliens[idx as usize];
                    al.worldy = -(100i16 << BOSSA_SCALE);
                    al.count = BOSSA_CUP_RETURN_TIME;
                    al.stratstate = BOSSA_CUP_STATE_RETURN;
                    add_player_z(g, idx);
                    return;
                }
                if g.objs.aliens[idx as usize].sflags2 & BOSSA_CUP_FLAG_FIRED != 0
                    && g.vars.gameframe % 12 == 0
                {
                    let pitch = strat_pitch_toward(&me, &pl);
                    let yaw = strat_angle_xz(&me, &pl);
                    let p = player_idx(g).unwrap();
                    let _ = boss1_fire_hmissile1(g, idx, p, 0, 0, 0, pitch, yaw);
                }
            }
            strat_speed_to(&mut g.objs.aliens[idx as usize], 45, 1);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotz = al.rotz.wrapping_add(28);
            }
            achase_angle(&mut g.objs.aliens[idx as usize].rotx, 0, 2);
            strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
            strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
            add_player_z(g, idx);
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.count > 0 {
                    al.count -= 1;
                }
            }
            if g.objs.aliens[idx as usize].count == 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.count = BOSSA_CUP_RETURN_TIME;
                al.stratstate = BOSSA_CUP_STATE_RETURN;
            }
            return;
        }
        BOSSA_CUP_STATE_RETURN => {
            g.objs.aliens[idx as usize].sflags |= ASF_NOHITAFFECT;
            bossa_cup_chase_home(g, idx, 110i16 << BOSSA_SCALE);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotz = al.rotz.wrapping_add(20);
            }
            achase_angle(&mut g.objs.aliens[idx as usize].rotx, DEG90.wrapping_neg(), 3);
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.count > 0 {
                    al.count -= 1;
                }
            }
            if g.objs.aliens[idx as usize].count == 0 {
                g.objs.aliens[mother_idx as usize].sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
                g.objs.aliens[idx as usize].stratstate = BOSSA_CUP_STATE_UP;
            }
        }
        BOSSA_CUP_STATE_DOWN => {
            let turret = bossa_linked_turret(g, idx);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags |= ASF_NOHITAFFECT;
                if al.animframe > 0 && g.vars.gameframe & 1 == 0 {
                    al.animframe -= 1;
                }
            }
            bossa_cup_chase_home(g, idx, 15i16 << BOSSA_SCALE);
            achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 3);
            achase_angle(&mut g.objs.aliens[idx as usize].rotx, DEG90.wrapping_neg(), 3);
            if let Some(t) = turret {
                let al = &mut g.objs.aliens[t as usize];
                al.sflags |= ASF_NOHITAFFECT;
                al.hp = BOSSA_TURRET_HP;
            }
        }
        _ => {
            g.objs.aliens[idx as usize].stratstate = BOSSA_CUP_STATE_COVER;
            bossa_cup_set_home(g, idx, 15i16 << BOSSA_SCALE);
        }
    }

    add_player_z(g, idx);
}

/// C `bossA_cupL_init` (strat_enemy.c:3304).
fn bossa_cupL_init(g: &mut Game, idx: u16) {
    bossa_cup_init_common(g, idx, BOSSA_CHILD_TURRET_L);
    let s = sid(g, bossa_cup_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossA_cupM_init` (strat_enemy.c:3309).
fn bossa_cupM_init(g: &mut Game, idx: u16) {
    bossa_cup_init_common(g, idx, BOSSA_CHILD_TURRET_M);
    let s = sid(g, bossa_cup_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossA_cupR_init` (strat_enemy.c:3314).
fn bossa_cupR_init(g: &mut Game, idx: u16) {
    bossa_cup_init_common(g, idx, BOSSA_CHILD_TURRET_R);
    let s = sid(g, bossa_cup_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossA_parent_continue` (strat_enemy.c:3319).
fn bossa_parent_continue(g: &mut Game, idx: u16) {
    if bossa_live_turret_count(g, idx) == 0 && bossa_live_cup_count(g, idx) == 0 {
        bossaexp_init(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.worldx > 210 && al.vx < 0 {
            al.worldx = al.worldx.wrapping_add(al.vx);
            al.vx += 1;
            if al.vx > 0 {
                al.vx = 0;
            }
        }
    }
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

/// C `bossAup_init` (strat_enemy.c:3341).
fn bossaup_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossaup_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = 30;
    play_se(g, 0x73);
}

/// C `bossAup_strat` (strat_enemy.c:3350).
fn bossaup_strat(g: &mut Game, idx: u16) {
    bossa_set_all_cup_state(g, idx, BOSSA_CUP_STATE_UP);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        bossaattack_init(g, idx);
        return;
    }
    bossa_parent_continue(g, idx);
}

/// C `bossAcover_init` (strat_enemy.c:3365).
fn bossacover_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossacover_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sbyte2 = 50;
    play_se(g, 0x72);
}

/// C `bossAcover_strat` (strat_enemy.c:3374).
fn bossacover_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
    }
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 > 20 {
        bossa_set_all_cup_state(g, idx, BOSSA_CUP_STATE_DOWN);
    } else {
        if sb2 == 19 {
            play_se(g, 0x73);
        }
        bossa_set_all_cup_state(g, idx, BOSSA_CUP_STATE_UP);
    }
    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        bossaattack_init(g, idx);
        return;
    }
    bossa_parent_continue(g, idx);
}

/// C `bossAattack_init` (strat_enemy.c:3396).
fn bossaattack_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossaattack_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.sbyte2 = 3;
        al.sbyte3 = 0;
        al.sflags4 |= BOSSA_PARENT_FLAG_ATTACK_DONE;
    }
    bossa_set_all_cup_state(g, idx, BOSSA_CUP_STATE_UP);
}

/// C `bossAattack_strat` (strat_enemy.c:3407).
fn bossaattack_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags4 & BOSSA_PARENT_FLAG_ATTACK_DONE != 0 {
        g.objs.aliens[idx as usize].sflags4 &= !BOSSA_PARENT_FLAG_ATTACK_DONE;
        let live_cups = bossa_live_cup_count(g, idx);
        if live_cups == 0 {
            bossacover_init(g, idx);
            return;
        }
        if g.objs.aliens[idx as usize].sbyte2 == 0 {
            bossacover_init(g, idx);
            return;
        }
        if let Some(cup) = bossa_pick_live_cup(g, idx) {
            let al = &mut g.objs.aliens[cup as usize];
            al.stratstate = if live_cups == 2 {
                BOSSA_CUP_STATE_IROTATE
            } else {
                BOSSA_CUP_STATE_GO
            };
            al.sflags2 &= !BOSSA_CUP_FLAG_FIRED;
            al.count = BOSSA_CUP_GO_TIME;
            g.objs.aliens[idx as usize].sbyte2 -= 1;
        }
    }

    if g.vars.gameframe % 5 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte3 = (al.sbyte3 + 1) % 3;
    }
    bossa_update_turret_targets(g, idx);

    if let Some(pl) = player(g) {
        if g.objs.aliens[idx as usize].sbyte3 == 2 {
            let phase = (g.vars.gameframe % BOSSA_PARENT_MISSILE_PERIOD) as u8;
            let roty = g.objs.aliens[idx as usize].roty;
            let p = player_idx(g).unwrap();
            let _ = &pl;
            if phase == 20 {
                let _ = boss1_fire_hmissile1(
                    g, idx, p, 0, -(25i16 << BOSSA_SCALE), 0, 0, roty.wrapping_sub(DEG22),
                );
            } else if phase == 25 {
                let _ = boss1_fire_hmissile1(g, idx, p, 0, -(25i16 << BOSSA_SCALE), 0, 0, roty);
            } else if phase == 30 {
                let _ = boss1_fire_hmissile1(
                    g, idx, p, 0, -(25i16 << BOSSA_SCALE), 0, 0, roty.wrapping_add(DEG22),
                );
            }
        }
    }

    bossa_parent_continue(g, idx);
}

/// C `bossA_strat` (strat_enemy.c:3462).
fn bossa_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].roty != DEG180 {
        g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(1);
        bossa_parent_continue(g, idx);
        return;
    }
    bossaup_init(g, idx);
}

/// C `bossAexp_init` (strat_enemy.c:3476).
fn bossaexp_init(g: &mut Game, idx: u16) {
    bossa_release_children(g, idx);
    let s = sid(g, bossaexp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.collstratptr = None;
        al.expstratptr = None;
        al.sflags |= ASF_COLLDISABLE;
        al.flags |= AFEXP;
        al.count = BOSSA_EXP_FRAMES;
    }
    set_bossflags(g, bossflags(g) | BF_DYING);
    play_se(g, 0x10);
}

/// C `bossAexp_strat` (strat_enemy.c:3492).
fn bossaexp_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(DEG90 / 24);
    }
    add_player_z(g, idx);

    if !strat_count_down(&mut g.objs.aliens[idx as usize]) {
        return;
    }

    g.vars.gameflags |= GF_BOSSDEAD;
    set_bossflags(g, bossflags(g) & !(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3));
    set_bossmaxhp(g, 0);
    g.objs.aldead = 1;
}

/// C `Strat_BossA_Init` (strat_enemy.c:3510).
pub fn strat_bossa_init(g: &mut Game, idx: u16) {
    let s_strat = sid(g, bossa_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, bossaexp_init);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_BOSS_A_2;
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
    }
    set_hard_vars(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG11;
        al.vx = -20;
        al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
        al.sflags |= ASF_SHADOW;
        al.sbyte2 = 0;
        al.sbyte3 = 0;
        al.sflags4 &= !BOSSA_PARENT_FLAG_ATTACK_DONE;
    }
    g.vars.gameflags &= !GF_BOSSDEAD;
    set_bossflags(g, bossflags(g) & !(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3));
    set_bossmaxhp(
        g,
        (BOSSA_TURRET_HP as u16 * 3) + (BOSSA_CUP_HP as u16 * 3),
    );
    g.vars.meters = 1;

    let _ = bossa_spawn_child(g, idx, BOSSA_CHILD_TURRET_L, bossa_turretL_init);
    let _ = bossa_spawn_child(g, idx, BOSSA_CHILD_TURRET_M, bossa_turretM_init);
    let _ = bossa_spawn_child(g, idx, BOSSA_CHILD_TURRET_R, bossa_turretR_init);
    let _ = bossa_spawn_child(g, idx, BOSSA_CHILD_CUP_L, bossa_cupL_init);
    let _ = bossa_spawn_child(g, idx, BOSSA_CHILD_CUP_M, bossa_cupM_init);
    let _ = bossa_spawn_child(g, idx, BOSSA_CHILD_CUP_R, bossa_cupR_init);
    bossa_update_turret_targets(g, idx);
    play_se(g, 0x83);
}
// EB_PART_2_END

// ============================================================
// EB_PART_3_BEGIN (strat_enemy.c:7334-9135 — spacepilon, bossF, title)
// ============================================================

/// C `spacepilon_rotate_z` (strat_enemy.c:7368): rotate (x,y) by 8-bit angle.
fn spacepilon_rotate_z(in_x: i16, in_y: i16, angle: u8) -> (i16, i16) {
    let s = strat_sin(angle);
    let c = strat_cos(angle);
    let out_x = (in_x as f32 * c - in_y as f32 * s) as i16;
    let out_y = (in_x as f32 * s + in_y as f32 * c) as i16;
    (out_x, out_y)
}

/// C `spacepilonP_strat` (strat_enemy.c:7378).
fn spacepilonP_strat(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    if !g.objs.aliens[mother_idx as usize].active {
        g.objs.aldead = 1;
        return;
    }

    let mother = g.objs.aliens[mother_idx as usize];
    let relposy = g.objs.aliens[idx as usize].relposy as i8;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = mother.rotz;
        al.rotz = al.rotz.wrapping_add(al.sbyte2);
    }
    let rotz = g.objs.aliens[idx as usize].rotz;
    let (off_x, off_y) = spacepilon_rotate_z(0, relposy as i16, rotz);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = mother
            .worldx
            .wrapping_add(off_x.wrapping_shl(SPACEPILON_PILON_ORBIT_SCALE));
        al.worldy = mother
            .worldy
            .wrapping_add(off_y.wrapping_shl(SPACEPILON_PILON_ORBIT_SCALE));
        al.worldz = mother.worldz;
    }

    match g.objs.aliens[idx as usize].stratstate {
        0 => {
            let tgt = SPACEPILON_PILON_RELPOSY_TGT;
            let diff = relposy.wrapping_sub(tgt);
            let mut step = diff >> 3;
            if step == 0 {
                step = if diff > 0 {
                    1
                } else if diff < 0 {
                    -1
                } else {
                    0
                };
            }
            if diff != 0 {
                g.objs.aliens[idx as usize].relposy = relposy.wrapping_sub(step) as u8;
            }
        }
        1 => {
            g.objs.aliens[mother_idx as usize].sflags |= ASF_COLLDISABLE;
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.relposy = (al.relposy as i8).wrapping_add(SPACEPILON_PILON_EXTEND_STEP) as u8;
                if al.sbyte3 > 0 {
                    al.sbyte3 -= 1;
                }
            }
            if g.objs.aliens[idx as usize].sbyte3 == 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratstate += 1;
                al.sbyte3 = SPACEPILON_PILON_SBYTE3_INIT;
                strat_trig_se(g, 0x2C);
            }
        }
        2 => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.relposy = (al.relposy as i8).wrapping_add(SPACEPILON_PILON_RETRACT_STEP) as u8;
                if al.sbyte3 > 0 {
                    al.sbyte3 -= 1;
                }
            }
            if g.objs.aliens[idx as usize].sbyte3 == 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.stratstate = 0;
                al.sbyte3 = SPACEPILON_PILON_SBYTE3_INIT;
                g.objs.aliens[mother_idx as usize].sflags &= !ASF_COLLDISABLE;
            }
        }
        _ => {
            g.objs.aliens[idx as usize].stratstate = 0;
        }
    }
}

/// C `spacepilonP_init` (strat_enemy.c:7467).
fn spacepilonP_init(g: &mut Game, idx: u16) {
    let s = sid(g, spacepilonP_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.type_ &= !ATZREMOVE;
    al.sbyte3 = SPACEPILON_PILON_SBYTE3_INIT;
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    al.relposy = SPACEPILON_PILON_RELPOSY_INIT as u8;
}

/// C `spacepilon_spawn_pilon` (strat_enemy.c:7481).
fn spacepilon_spawn_pilon(g: &mut Game, mother: u16, child_num: u8, orbit_rot: i8) -> Option<u16> {
    let child = strat_make_obj(g, SH_PILON)?;
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.aldead = 1;
        return None;
    }
    g.objs.aliens[child as usize].collflags |= ACF_COLLTYPE4;
    spacepilonP_init(g, child);
    g.objs.aliens[child as usize].sbyte2 = orbit_rot as u8;
    Some(child)
}

/// C `spacepiloncol_init` (strat_enemy.c:7502).
fn spacepiloncol_init(g: &mut Game, idx: u16) {
    strat_trig_se(g, 0x27);
    for c in 1u8..=3 {
        if let Some(child) = boss_find_child_obj(g, idx, c) {
            g.objs.aliens[child as usize].stratstate = 1;
        }
    }
    g.objs.aliens[idx as usize].sbyte2 = 10;
    if let Some(exp) = make_large_exp_obj(g, idx) {
        g.objs.aliens[exp as usize].sflags2 |= ASF2_NOEXPSND;
    }
    strat_hit_flash(g, idx);
}

/// C `spacepilonexp_init` (strat_enemy.c:7537).
fn spacepilonexp_init(g: &mut Game, idx: u16) {
    for c in 1u8..=3 {
        if let Some(child) = boss_find_child_obj(g, idx, c) {
            g.objs.aliens[child as usize].active = false;
            obj_free(g, child);
        }
    }
    strat_explode(g, idx);
}

/// C `spacepilon_strat` (strat_enemy.c:7566).
fn spacepilon_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].stratstate == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_COLLDISABLE;
        al.worldx = strat_chase_proportional(al.worldx, al.vx, 4);
        al.worldy = strat_chase_proportional(al.worldy, al.vy, 4);
        al.worldz = strat_chase_proportional(al.worldz, al.vz, 4);
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
        if al.sbyte2 == 0 {
            al.sflags &= !ASF_COLLDISABLE;
            al.stratstate += 1;
            al.sbyte2 = 1;
        }
    }

    if g.objs.aliens[idx as usize].stratstate == 1 {
        {
            let al = &mut g.objs.aliens[idx as usize];
            if al.sbyte2 > 0 {
                al.sbyte2 -= 1;
            }
            if al.sbyte2 == 0 {
                al.sbyte2 = 1;
                al.rotz = al.rotz.wrapping_add(SPACEPILON_ROT_STEP);
            }
        }
        if g.vars.gameframe & SPACEPILON_FIRE_MASK == 0 {
            if let Some(pl) = player(g) {
                let me = g.objs.aliens[idx as usize];
                let fire_pitch = strat_pitch_toward(&me, &pl);
                let fire_yaw = strat_angle_xz(&me, &pl);
                let pptr = player_idx(g).map(|i| i + 1).unwrap_or(0);
                if let Some(shot) = strat_spawn_projectile(
                    g,
                    Some(idx),
                    0,
                    0,
                    0,
                    fire_pitch,
                    fire_yaw,
                    HPLASMA_SPEED,
                    HPLASMA_LIFE,
                    HPLASMA_AP,
                    ACF_COLLTYPE4,
                ) {
                    g.objs.aliens[shot as usize].ptr = pptr;
                }
            }
        }
    }

    if g.objs.aliens[idx as usize].sbyte4 == 0 {
        g.objs.aliens[idx as usize].type_ |= ATZREMOVE;
        for c in 1u8..=3 {
            if let Some(child) = boss_find_child_obj(g, idx, c) {
                g.objs.aliens[child as usize].type_ |= ATZREMOVE;
            }
        }
        return;
    }
    g.objs.aliens[idx as usize].sbyte4 -= 1;

    add_player_z(g, idx);
    {
        let vz_add = g.vars.pviewvelz;
        let al = &mut g.objs.aliens[idx as usize];
        al.vz = al.vz.wrapping_add(vz_add);
    }
}

/// C `Strat_Spacepilon_Init` (strat_enemy.c:7660).
pub fn strat_spacepilon_init(g: &mut Game, idx: u16) {
    let _ = spacepilon_spawn_pilon(g, idx, 1, SPACEPILON_CHILD1_ROT);
    let _ = spacepilon_spawn_pilon(g, idx, 2, SPACEPILON_CHILD2_ROT);
    let _ = spacepilon_spawn_pilon(g, idx, 3, 0);

    let s_strat = sid(g, spacepilon_strat);
    let s_coll = sid(g, spacepiloncol_init);
    let s_exp = sid(g, spacepilonexp_init);

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.type_ &= !ATZREMOVE;
        let phase = idx as u8;
        al.worldx = al.worldx.wrapping_add((phase.wrapping_mul(37)) as i8 as i16);
        al.worldy = al.worldy.wrapping_add((phase.wrapping_mul(53)) as i8 as i16);
        al.worldz = al.worldz.wrapping_add((phase.wrapping_mul(17)) as i8 as i16);
        al.vx = al.worldx;
        al.vy = al.worldy;
        al.vz = al.worldz;
    }
    if let Some(pl) = player(g) {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = pl.worldx;
        al.worldy = pl.worldy;
        al.worldz = pl.worldz.wrapping_add(SPACEPILON_SPAWN_ZOFF);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
        al.sbyte2 = SPACEPILON_ZOOMIN_CNT;
        al.hp = SPACEPILON_HP;
        al.ap = SPACEPILON_AP;
        al.sbyte4 = SPACEPILON_RELEASE_CNT;
        al.collflags |= ACF_COLLTYPE4;
        al.stratstate = 0;
    }
    strat_trig_se(g, 0x2C);
}

// --- bossF helpers ---

/// C `bossF_spawn_child` (strat_enemy.c:7802).
fn bossf_spawn_child(g: &mut Game, mother: u16, shape: u16, child_num: u8, init_fn: StrategyFn) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.shape = shape;
        al.collflags |= ACF_COLLTYPE3;
    }
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        obj_free(g, child);
        return None;
    }
    init_fn(g, child);
    Some(child)
}

/// C `bossF_spawn_child_pos` (strat_enemy.c:7828).
#[allow(clippy::too_many_arguments)]
fn bossf_spawn_child_pos(
    g: &mut Game,
    mother: u16,
    shape: u16,
    child_num: u8,
    offx: i16,
    offy: i16,
    offz: i16,
    init_fn: StrategyFn,
) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    let m = g.objs.aliens[mother as usize];
    {
        let al = &mut g.objs.aliens[child as usize];
        al.shape = shape;
        al.collflags |= ACF_COLLTYPE3;
        al.worldx = m.worldx.wrapping_add(offx);
        al.worldy = m.worldy.wrapping_add(offy);
        al.worldz = m.worldz.wrapping_add(offz);
    }
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        obj_free(g, child);
        return None;
    }
    init_fn(g, child);
    Some(child)
}

/// C `bossF_rotated_offset` (strat_enemy.c:7860): place `dst` at ref pos +
/// (offx,offy,offz) rotated by ref's rotz/rotx/roty then scaled.
fn bossf_rotated_offset(g: &mut Game, dst: u16, refa: &Alien, offx: i16, offy: i16, offz: i16, scale: u32) {
    let mut fx = offx as f32;
    let mut fy = offy as f32;
    let mut fz = offz as f32;

    let sz = strat_sin(refa.rotz);
    let cz = strat_cos(refa.rotz);
    let tx = fx * cz - fy * sz;
    let ty = fx * sz + fy * cz;
    fx = tx;
    fy = ty;

    let sx = strat_sin(refa.rotx);
    let cx = strat_cos(refa.rotx);
    let ty = fy * cx - fz * sx;
    fz = fy * sx + fz * cx;
    fy = ty;

    let sy = strat_sin(refa.roty);
    let cy = strat_cos(refa.roty);
    let tx = fx * cy + fz * sy;
    fz = -fx * sy + fz * cy;
    fx = tx;

    if scale > 0 {
        let f = (1i32 << scale) as f32;
        fx *= f;
        fy *= f;
        fz *= f;
    }

    let al = &mut g.objs.aliens[dst as usize];
    al.worldx = refa.worldx.wrapping_add(fx.round() as i16);
    al.worldy = refa.worldy.wrapping_add(fy.round() as i16);
    al.worldz = refa.worldz.wrapping_add(fz.round() as i16);
}

/// C `bossFCsmoke_srou` (strat_enemy.c:7919).
fn bossfcsmoke_srou(g: &mut Game, idx: u16) {
    if g.vars.gameframe % 4 != 0 {
        return;
    }
    play_se(g, 0x23);
    let Some(smoke) = strat_make_obj(g, 0) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    bossf_rotated_offset(g, smoke, &me, 0, -20, 0, BOSSF_SCALE);
    let s_strat = sid(g, delayexplode_strat);
    let s_exp = sid(g, strat_explode);
    let al = &mut g.objs.aliens[smoke as usize];
    al.shape = EXPSHAPE_LARGE;
    al.count = 10;
    al.sflags |= ASF_COLLDISABLE;
    al.sflags2 |= ASF2_NOEXPSND;
    al.stratptr = Some(s_strat);
    al.expstratptr = Some(s_exp);
}

/// C `bossF_fire_hplasma` (strat_enemy.c:7947).
fn bossf_fire_hplasma(g: &mut Game, idx: u16, wpx: i16, wpy: i16) {
    let Some(pl) = player(g) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let yaw = strat_angle_xz(&me, &pl);
    let pitch = strat_pitch_toward(&me, &pl);
    let pptr = player_idx(g).map(|i| i + 1).unwrap_or(0);
    if let Some(shot) = strat_spawn_projectile(
        g,
        Some(idx),
        wpx,
        wpy,
        0,
        pitch,
        yaw,
        HPLASMA_SPEED,
        HPLASMA_LIFE,
        HPLASMA_AP,
        ACF_COLLTYPE4,
    ) {
        g.objs.aliens[shot as usize].ptr = pptr;
    }
}

/// C `bossF_remove_child` (strat_enemy.c:7974).
fn bossf_remove_child(g: &mut Game, mother: u16, child_num: u8) {
    if let Some(child) = boss_find_child_obj(g, mother, child_num) {
        boss_clear_child_link(g, child);
        obj_free(g, child);
        boss_prune_family_links(g, mother);
    }
}

/// C `Strat_BossF_Init` (strat_enemy.c:7996).
pub fn strat_bossf_init(g: &mut Game, idx: u16) {
    set_hard_vars(&mut g.objs.aliens[idx as usize]);
    let s_strat = sid(g, bossfc_strat);
    let s_coll = sid(g, strat_hit_flash);
    let s_exp = sid(g, strat_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.collstratptr = Some(s_coll);
        al.expstratptr = Some(s_exp);
    }
    if let Some(childa) = bossf_spawn_child(g, idx, SH_BOSS_F_1, BOSSF_CHILD_A, bossfa_istrat) {
        copy_pos(g, childa, idx);
        let al = &mut g.objs.aliens[childa as usize];
        al.worldx = al.worldx.wrapping_add(400);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.collflags |= ACF_COLLTYPE3;
        al.stratstate = 0;
        al.animframe = 0;
        al.sflags |= ASF_NOHITAFFECT;
    }
    set_bossmaxhp(g, 0);
    g.vars.meters = 1;
    g.vars.gameflags &= !GF_BOSSDEAD;
    set_bossflags(g, bossflags(g) & !(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3));
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte2 = 200;
        al.type_ |= ATGND;
    }
}

/// C `bossFC_strat` (strat_enemy.c:8047).
fn bossfc_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte2 == 200 - 60 {
        if let Some(childb) = bossf_spawn_child(g, idx, SH_BOSS_F_2, BOSSF_CHILD_B, bossfb_istrat) {
            copy_pos(g, childb, idx);
            let al = &mut g.objs.aliens[childb as usize];
            al.worldx = al.worldx.wrapping_sub(400);
        }
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
            if al.sbyte2 == 0 {
                al.sbyte2 = 1;
            }
        } else {
            al.sbyte2 = 1;
        }
    }

    // do_states
    let state = g.objs.aliens[idx as usize].stratstate;
    if state == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = -10;
        if al.worldy < BOSSF_SPACE_VIEWCY + 300 {
            al.stratstate = 1;
            al.count = 4;
        }
    } else if state == 1 {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(1);
        if al.vy == 0 {
            al.stratstate = 2;
            al.count = 4;
            al.sbyte3 = 100;
        }
    } else if state == 2 {
        let reached = achase_angle(&mut g.objs.aliens[idx as usize].roty, 0, 4);
        if !reached && g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
            play_se(g, 0x8E);
        }
        g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
        {
            let al = &mut g.objs.aliens[idx as usize];
            if al.sbyte3 > 0 {
                al.sbyte3 -= 1;
            }
        }
        if g.objs.aliens[idx as usize].sbyte3 == 0 {
            bossfc2start_init(g, idx);
            return;
        }
    }

    // movement
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(al.vy as u8);
    }
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// C `bossFC2start_init` (strat_enemy.c:8135).
fn bossfc2start_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].type_ &= !ATGND;
    bossf_remove_child(g, idx, BOSSF_CHILD_A);
    bossf_remove_child(g, idx, BOSSF_CHILD_B);
    g.objs.aliens[idx as usize].shape = SH_BOSS_F_4;

    let s = BOSSF_SCALE;
    let _ = bossf_spawn_child_pos(g, idx, SH_BOSS_F_8, BOSSF_CHILD_TUR1, -20i16 << s, -10i16 << s, 30i16 << s, bossftur1_istrat);
    let _ = bossf_spawn_child_pos(g, idx, SH_BOSS_F_8, BOSSF_CHILD_TUR2, -20i16 << s, -10i16 << s, -10i16 << s, bossftur2_istrat);
    let _ = bossf_spawn_child_pos(g, idx, SH_BOSS_F_9, BOSSF_CHILD_TUR3, 20i16 << s, -10i16 << s, 30i16 << s, bossftur3_istrat);
    let _ = bossf_spawn_child_pos(g, idx, SH_BOSS_F_9, BOSSF_CHILD_TUR4, 20i16 << s, -10i16 << s, -10i16 << s, bossftur4_istrat);
    let _ = bossf_spawn_child_pos(g, idx, SH_BOSS_F_8, BOSSF_CHILD_TUR5, -20i16 << s, -10i16 << s, 10i16 << s, bossftur5_istrat);
    let _ = bossf_spawn_child_pos(g, idx, SH_BOSS_F_9, BOSSF_CHILD_TUR6, 20i16 << s, -10i16 << s, 10i16 << s, bossftur6_istrat);

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte2 = 0;
        al.type_ |= ATGND;
        al.collflags |= ACF_COLLTYPE4;
        al.type_ &= !ATZREMOVE;
        al.roty = DEG180;
    }
    bossfc2b_init(g, idx);
}

/// C `bossFC2_init` (strat_enemy.c:8191).
fn bossfc2_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].rotz = DEG90.wrapping_neg();
    bossfc2b_init(g, idx);
}

/// C `bossFC2b_init` (strat_enemy.c:8203).
fn bossfc2b_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossfc2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sflags2 &= !ASF2_SFLAG1;
    al.sflags4 &= !ASF4_SFLAG8;
}

/// C `bossFC2_strat` (strat_enemy.c:8220).
fn bossfc2_strat(g: &mut Game, idx: u16) {
    {
        let reached = achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 5);
        if reached && g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 == 0 {
            g.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8;
            play_se(g, 0x58);
        }
    }

    if g.vars.gameframe & 1 == 0 {
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 6);
        let al = &mut g.objs.aliens[idx as usize];
        if al.rotx != DEG5 {
            al.rotx = al.rotx.wrapping_add(1);
        }
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let player_in_front = pl.worldz > me.worldz;
        if !player_in_front {
            let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
            if zdist > 4000 {
                bossfc3_init(g, idx);
                return;
            }
            if zdist >= 2000 && me.sflags2 & ASF2_SFLAG1 == 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags2 |= ASF2_SFLAG1;
                al.rotx = 0;
                al.worldy = BOSSF_SPACE_VIEWCY - 500;
            }
        }
    }

    bossfc2_cont(g, idx);
}

/// C `bossFC2_cont` (strat_enemy.c:8284).
fn bossfc2_cont(g: &mut Game, idx: u16) {
    let sb2 = g.objs.aliens[idx as usize].sbyte2;
    if sb2 >= 6 {
        bossfcdie_init(g, idx);
        return;
    }
    if sb2 >= 3 {
        bossfcsmoke_srou(g, idx);
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
        if zdist >= 600 {
            if g.vars.gameframe & 31 == 10 {
                let wpx = ((-20i16) >> WEAPON_SCALE) << BOSSF_SCALE;
                let wpy = ((-20i16) >> WEAPON_SCALE) << BOSSF_SCALE;
                bossf_fire_hplasma(g, idx, wpx, wpy);
            } else if g.vars.gameframe & 31 == 5 {
                let wpx = (20i16 << BOSSF_SCALE) >> WEAPON_SCALE;
                let wpy = ((-20i16) << BOSSF_SCALE) >> WEAPON_SCALE;
                bossf_fire_hplasma(g, idx, wpx, wpy);
            }
        }
    }

    bossfc2_cont2(g, idx);
}

/// C `bossFC2_cont2` (strat_enemy.c:8326).
fn bossfc2_cont2(g: &mut Game, idx: u16) {
    strat_speed_to(&mut g.objs.aliens[idx as usize], 50, 1);
    bossfc2_cont3(g, idx);
}

/// C `bossFC2_cont3` (strat_enemy.c:8340).
fn bossfc2_cont3(g: &mut Game, idx: u16) {
    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].vx = 0;
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// C `bossFC3_init` (strat_enemy.c:8358).
fn bossfc3_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossfc3_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.sflags2 &= !ASF2_SFLAG1;
    al.sflags4 &= !ASF4_SFLAG8;
    al.rotz = DEG90.wrapping_neg();
}

/// C `bossFC3_strat` (strat_enemy.c:8372).
fn bossfc3_strat(g: &mut Game, idx: u16) {
    {
        let reached = achase_angle(&mut g.objs.aliens[idx as usize].roty, 0, 5);
        if reached && g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 == 0 {
            g.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8;
            play_se(g, 0x58);
        }
    }

    if g.vars.gameframe & 1 == 0 {
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 6);
        let al = &mut g.objs.aliens[idx as usize];
        if al.rotx != DEG5 {
            al.rotx = al.rotx.wrapping_add(1);
        }
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let player_in_front = pl.worldz < me.worldz;
        if !player_in_front {
            let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
            if zdist > 4000 {
                bossfc2_init(g, idx);
                return;
            }
            if zdist >= 2000 && me.sflags2 & ASF2_SFLAG1 == 0 {
                let al = &mut g.objs.aliens[idx as usize];
                al.sflags2 |= ASF2_SFLAG1;
                al.rotx = 0;
                al.worldy = BOSSF_SPACE_VIEWCY - 500;
            }
        }
    }

    bossfc2_cont(g, idx);
}

/// C `bossFCdie_init` (strat_enemy.c:8427).
fn bossfcdie_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossfcdie_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `bossFCdie_strat` (strat_enemy.c:8439).
fn bossfcdie_strat(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 1 == 0 {
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 6);
    }
    if g.vars.gameframe & 1 == 0 {
        if let Some(exp) = make_medium_exp_obj(g, idx) {
            addrnd2pos_xy(g, exp);
        }
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
        if zdist < 1000 {
            bossfcdie2_init(g, idx);
            return;
        }
    }

    bossfcsmoke_srou(g, idx);
    bossfc2_cont2(g, idx);
}

/// C `bossFCdie2_init` (strat_enemy.c:8476).
fn bossfcdie2_init(g: &mut Game, idx: u16) {
    let s = sid(g, bossfcdie2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s);
        al.rotx = 0;
        al.sflags4 &= !ASF4_SFLAG8;
    }
    let sf = wm8(g, ebwm::STRATFLAGS);
    wm8_set(g, ebwm::STRATFLAGS, sf | SF_NOFIRING);
}

/// C `bossFCdie2_strat` (strat_enemy.c:8499).
fn bossfcdie2_strat(g: &mut Game, idx: u16) {
    if g.vars.gameframe & 1 == 0 {
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 6);
    }

    if let Some(exp) = make_large_exp_obj(g, idx) {
        let rx = (sfrtl_random(g) & 0xFF) as i8;
        let ry = (sfrtl_random(g) & 0xFF) as i8;
        let rz = (sfrtl_random(g) & 0xFF) as i8;
        let al = &mut g.objs.aliens[exp as usize];
        al.sflags4 |= ASF4_NOPOLYEXP;
        al.worldx = al.worldx.wrapping_add(rx as i16);
        al.worldy = al.worldy.wrapping_add(ry as i16);
        al.worldz = al.worldz.wrapping_add(rz as i16);
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
        if zdist >= 4000 {
            if g.vars.gameframe & 1 == 0 {
                if let Some(fexp) = make_fol_exp_obj(g, idx) {
                    g.objs.aliens[fexp as usize].sflags4 |= ASF4_NOPOLYEXP;
                    addrnd2pos_xy(g, fexp);
                }
            }

            if g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 == 0 {
                g.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8;
                play_music(g, 0xF0);
                play_se(g, 0x96);
            }

            g.objs.aliens[idx as usize].rotz = g.objs.aliens[idx as usize].rotz.wrapping_add(4);

            if g.objs.aliens[idx as usize].rotx == DEG90 {
                g.world.lastplayz = g.objs.aliens[idx as usize].worldz;
                g.world.lastzchange = 0;
                g.vars.mapcnt = 0;

                if g.vars.pshipflags2 & PSF2_TURN180 != 0 {
                    g.vars.pshipflags2 &= !PSF2_TURN180;
                    if let Some(p) = player_idx(g) {
                        let al = &mut g.objs.aliens[p as usize];
                        al.vz = al.vz.wrapping_neg();
                        al.worldx = al.worldx.wrapping_neg();
                    }
                    g.vars.pviewvelz = g.vars.pviewvelz.wrapping_neg();
                    g.vars.set_sv_i16(sv::PLAYER_TURNROT, 0);
                }

                g.vars.gameflags |= GF_BOSSDEAD;
                set_bossflags(g, bossflags(g) & !(BF_DYING | BF_FLAG1 | BF_FLAG2 | BF_FLAG3));
                set_bossmaxhp(g, 0);
                let sf = wm8(g, ebwm::STRATFLAGS);
                wm8_set(g, ebwm::STRATFLAGS, sf & !SF_NOFIRING);
                g.objs.aldead = 1;
                return;
            }

            strat_speed_to(&mut g.objs.aliens[idx as usize], 60, 1);
            g.objs.aliens[idx as usize].rotx = g.objs.aliens[idx as usize].rotx.wrapping_add(1);
            bossfcsmoke_srou(g, idx);
            bossfc2_cont3(g, idx);
            return;
        }
    }

    if g.vars.gameframe % 4 == 0 {
        play_se(g, 0x21);
    }
    bossfcsmoke_srou(g, idx);
    bossfc2_cont3(g, idx);
}

// --- bossF turrets ---

fn bossftur_common_istrat(g: &mut Game, idx: u16, strat: StrategyFn, sbyte3: u8, hp: u8, exp: StrategyFn, sbyte2: u8) {
    let s_strat = sid(g, strat);
    let s_exp = sid(g, exp);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(s_strat);
        al.sbyte3 = sbyte3;
        al.hp = hp;
        al.ap = BOSSF_LAUNCHER_AP;
        al.expstratptr = Some(s_exp);
        al.sbyte2 = sbyte2;
    }
    bossftur_icont(g, idx);
}

/// C `bossFtur1_Istrat` (strat_enemy.c:8610).
fn bossftur1_istrat(g: &mut Game, idx: u16) {
    bossftur_common_istrat(g, idx, bossftur1_strat, 25, BOSSF_LAUNCHER2_HP, bossfexp1_istrat, 30);
}
/// C `bossFtur2_Istrat` (strat_enemy.c:8621).
fn bossftur2_istrat(g: &mut Game, idx: u16) {
    bossftur_common_istrat(g, idx, bossftur2_strat, 25, BOSSF_LAUNCHER_HP, bossfexp1_istrat, 60);
}
/// C `bossFtur3_Istrat` (strat_enemy.c:8632).
fn bossftur3_istrat(g: &mut Game, idx: u16) {
    bossftur_common_istrat(g, idx, bossftur3_strat, 50, BOSSF_LAUNCHER2_HP, bossfexp2_istrat, 90);
}
/// C `bossFtur4_Istrat` (strat_enemy.c:8643).
fn bossftur4_istrat(g: &mut Game, idx: u16) {
    bossftur_common_istrat(g, idx, bossftur4_strat, 50, BOSSF_LAUNCHER_HP, bossfexp2_istrat, 120);
}
/// C `bossFtur5_Istrat` (strat_enemy.c:8654).
fn bossftur5_istrat(g: &mut Game, idx: u16) {
    bossftur_common_istrat(g, idx, bossftur5_strat, 25, BOSSF_LAUNCHER_HP, bossfexp1_istrat, 150);
}
/// C `bossFtur6_Istrat` (strat_enemy.c:8665).
fn bossftur6_istrat(g: &mut Game, idx: u16) {
    bossftur_common_istrat(g, idx, bossftur6_strat, 50, BOSSF_LAUNCHER_HP, bossfexp2_istrat, 180);
}

/// C `bossFtur_Icont` (strat_enemy.c:8679).
fn bossftur_icont(g: &mut Game, idx: u16) {
    let s_coll = sid(g, strat_hit_flash);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.animframe = 0;
        al.type_ &= !ATZREMOVE;
        al.collflags |= ACF_COLLTYPE4;
        al.collstratptr = Some(s_coll);
    }
    let hp = g.objs.aliens[idx as usize].hp;
    if boss_get_mother_obj(g, idx).is_some() {
        set_bossmaxhp(g, bossmaxhp(g).wrapping_add(hp as u16));
    }
}

fn bossftur_pos_strat(g: &mut Game, idx: u16, offx: i16, offy: i16, offz: i16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    let mother = g.objs.aliens[mother_idx as usize];
    bossf_rotated_offset(g, idx, &mother, offx, offy, offz, BOSSF_SCALE);
    bossftur_cont(g, idx);
}

/// C `bossFtur1_strat` (strat_enemy.c:8704).
fn bossftur1_strat(g: &mut Game, idx: u16) {
    bossftur_pos_strat(g, idx, -20, -10, 30);
}
/// C `bossFtur2_strat` (strat_enemy.c:8713).
fn bossftur2_strat(g: &mut Game, idx: u16) {
    bossftur_pos_strat(g, idx, -20, -10, -10);
}
/// C `bossFtur3_strat` (strat_enemy.c:8722).
fn bossftur3_strat(g: &mut Game, idx: u16) {
    bossftur_pos_strat(g, idx, 20, -10, 30);
}
/// C `bossFtur4_strat` (strat_enemy.c:8731).
fn bossftur4_strat(g: &mut Game, idx: u16) {
    bossftur_pos_strat(g, idx, 20, -10, -10);
}
/// C `bossFtur5_strat` (strat_enemy.c:8740).
fn bossftur5_strat(g: &mut Game, idx: u16) {
    bossftur_pos_strat(g, idx, -20, -10, 10);
}
/// C `bossFtur6_strat` (strat_enemy.c:8749).
fn bossftur6_strat(g: &mut Game, idx: u16) {
    bossftur_pos_strat(g, idx, 20, -10, 10);
}

/// C `bossFtur_cont` (strat_enemy.c:8764).
fn bossftur_cont(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    if !g.objs.aliens[mother_idx as usize].active {
        g.objs.aldead = 1;
        return;
    }
    let mother = g.objs.aliens[mother_idx as usize];
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = mother.rotx;
        al.roty = mother.roty;
        al.rotz = mother.rotz;
    }
    if g.objs.aliens[idx as usize].hp == HARD_HP {
        return;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte2 > 0 {
            al.sbyte2 -= 1;
        }
        if al.sbyte2 == 0 {
            al.sflags2 ^= ASF2_SFLAG1;
            if al.sflags2 & ASF2_SFLAG1 != 0 {
                al.sbyte2 = 40;
            } else {
                al.sbyte2 = 70;
            }
        }
    }

    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 == 0 {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags |= ASF_NOHITAFFECT;
        if g.vars.gameframe & 1 == 0 && al.animframe > 0 {
            al.animframe -= 1;
        }
        return;
    }

    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_NOHITAFFECT;
        if g.vars.gameframe & 1 == 0 && al.animframe < 3 {
            al.animframe += 1;
        }
        if al.sbyte3 > 0 {
            al.sbyte3 -= 1;
        }
        if al.sbyte3 == 0 {
            al.sbyte3 = 50;
        }
    }

    if g.objs.aliens[idx as usize].sbyte3 > 20 {
        return;
    }
    if g.objs.aliens[idx as usize].sbyte2 <= 15 {
        return;
    }

    let Some(pl) = player(g) else {
        return;
    };
    let me = g.objs.aliens[idx as usize];
    let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
    if zdist < 1500 {
        return;
    }
    if g.vars.gameframe % 4 != 0 {
        return;
    }

    let yaw = strat_angle_xz(&me, &pl);
    let pitch = strat_pitch_toward(&me, &pl);
    let speed = strat_relslowelaser_speed(g);
    let _ = strat_spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        speed,
        RELSLOWELASERHOME_LIFE,
        RELSLOWELASERHOME_AP,
        ACF_COLLTYPE4,
    );
}

/// C `bossfexp1_Istrat` (strat_enemy.c:8877).
fn bossfexp1_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].shape = SH_BOSS_F_8A;
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        let m = &mut g.objs.aliens[mother as usize];
        m.sbyte2 = m.sbyte2.wrapping_add(1);
    }
    let _ = make_medium_exp_obj(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARD_HP;
    al.sflags |= ASF_COLLDISABLE;
}

/// C `bossfexp2_Istrat` (strat_enemy.c:8903).
fn bossfexp2_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].shape = SH_BOSS_F_9A;
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        let m = &mut g.objs.aliens[mother as usize];
        m.sbyte2 = m.sbyte2.wrapping_add(1);
    }
    let _ = make_medium_exp_obj(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARD_HP;
    al.sflags |= ASF_COLLDISABLE;
}

/// C `bossFA_Istrat` (strat_enemy.c:8926).
fn bossfa_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, bossfa_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.rotx = DEG90.wrapping_neg();
    al.vel = 50;
    al.stratstate = 0;
    al.animframe = 0;
}

/// C `bossFA_strat` (strat_enemy.c:8943).
fn bossfa_strat(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    if !g.objs.aliens[mother_idx as usize].active {
        g.objs.aldead = 1;
        return;
    }
    let mother = g.objs.aliens[mother_idx as usize];

    if mother.sflags2 & ASF2_SFLAG1 != 0 {
        let targ_z = mother.worldz.wrapping_add(10i16 << BOSSF_SCALE);
        let targ_y = mother.worldy.wrapping_add(-22i16 << BOSSF_SCALE);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldz = al.worldz.wrapping_add((targ_z - al.worldz) >> 2);
            al.worldx = al.worldx.wrapping_add((mother.worldx - al.worldx) >> 2);
            al.worldy = al.worldy.wrapping_add((targ_y - al.worldy) >> 2);
        }
        achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 3);
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 3);
        add_player_z(g, idx);
        return;
    }

    if g.objs.aliens[idx as usize].stratstate == 0
        && g.objs.aliens[idx as usize].worldy < BOSSF_SPACE_VIEWCY + 800
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratstate = 1;
        al.count = 4;
    }

    if g.objs.aliens[idx as usize].stratstate == 1 {
        achase_angle(&mut g.objs.aliens[idx as usize].rotx, 0, 4);
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.rotz = al.rotz.wrapping_add(2);
    }

    if !strat_points_positive_z(&g.objs.aliens[idx as usize]) && g.vars.gameframe % 4 == 0 {
        if let Some(pl) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let yaw = strat_angle_xz(&me, &pl);
            let pitch = strat_pitch_toward(&me, &pl);
            let speed = strat_relslowelaser_speed(g);
            let _ = strat_spawn_projectile(
                g, Some(idx),
                (-20i16 << BOSSF_SCALE) >> WEAPON_SCALE,
                (-20i16 << BOSSF_SCALE) >> WEAPON_SCALE,
                0, pitch, yaw, speed, RELSLOWELASERHOME_LIFE, RELSLOWELASERHOME_AP, ACF_COLLTYPE4,
            );
            let speed = strat_relslowelaser_speed(g);
            let _ = strat_spawn_projectile(
                g, Some(idx),
                (20i16 << BOSSF_SCALE) >> WEAPON_SCALE,
                (-20i16 << BOSSF_SCALE) >> WEAPON_SCALE,
                0, pitch, yaw, speed, RELSLOWELASERHOME_LIFE, RELSLOWELASERHOME_AP, ACF_COLLTYPE4,
            );
        }
    }

    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vz >>= 1;
    }
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// C `bossFB_Istrat` (strat_enemy.c:9025).
fn bossfb_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, bossfb_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.rotx = DEG90.wrapping_neg();
    al.vel = 30;
    al.stratstate = 0;
    al.animframe = 0;
}

/// C `bossFB_strat` (strat_enemy.c:9040).
fn bossfb_strat(g: &mut Game, idx: u16) {
    let Some(mother_idx) = boss_get_mother_obj(g, idx) else {
        g.objs.aldead = 1;
        return;
    };
    if !g.objs.aliens[mother_idx as usize].active {
        g.objs.aldead = 1;
        return;
    }
    let mother = g.objs.aliens[mother_idx as usize];

    if mother.sflags2 & ASF2_SFLAG1 != 0 {
        let targ_z = mother.worldz.wrapping_add(-40i16 << BOSSF_SCALE);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldz = al.worldz.wrapping_add((targ_z - al.worldz) >> 2);
            al.worldx = al.worldx.wrapping_add((mother.worldx - al.worldx) >> 2);
            al.worldy = al.worldy.wrapping_add((mother.worldy - al.worldy) >> 2);
        }
        achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 3);
        achase_angle(&mut g.objs.aliens[idx as usize].rotz, 0, 3);
        add_player_z(g, idx);
        return;
    }

    if g.objs.aliens[idx as usize].stratstate == 0
        && g.objs.aliens[idx as usize].worldy < BOSSF_SPACE_VIEWCY + 600
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratstate = 1;
        al.count = 4;
    }

    if g.objs.aliens[idx as usize].stratstate == 1 {
        achase_angle(&mut g.objs.aliens[idx as usize].rotx, 0, 4);
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_sub(2);
        al.rotz = al.rotz.wrapping_sub(1);
    }

    if let Some(pl) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let zdist = (me.worldz as i32 - pl.worldz as i32).abs() as i16;
        if zdist < 3000 {
            if g.objs.aliens[idx as usize].sflags4 & ASF4_SFLAG8 == 0 {
                play_se(g, 0x58);
                g.objs.aliens[idx as usize].sflags4 |= ASF4_SFLAG8;
            }
        } else {
            g.objs.aliens[idx as usize].sflags4 &= !ASF4_SFLAG8;
        }

        if zdist < 1400 && g.vars.gameframe % 4 == 0 {
            if let Some(mine) = strat_make_obj(g, 0) {
                copy_pos(g, mine, idx);
                let al = &mut g.objs.aliens[mine as usize];
                al.hp = 2;
                al.ap = 10;
                al.count = 60;
                al.collflags = ACF_COLLTYPE4;
                al.sflags |= ASF_COLLDISABLE;
                al.stratptr = None;
            }
        }
    }

    strat_gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vz >>= 1;
    }
    strat_apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

// --- Title screen spinning Arwing (TITLE.ASM tit_istrat) ---

/// C `strat_title_tick` (strat_enemy.c:9124).
fn strat_title_tick(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.roty = al.roty.wrapping_add(1);
}

/// C `Strat_Title_Init` (strat_enemy.c:9129).
pub fn strat_title_init(g: &mut Game, idx: u16) {
    let s = sid(g, strat_title_tick);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARD_HP;
    al.ap = 0;
    al.collflags = 0;
    al.roty = 0;
    al.stratptr = Some(s);
}
// EB_PART_3_END

// ============================================================
// Table-lane registration (C: the enemy_b-owned rows of
// `Strat_RegisterAll` + `Strat_RegisterAddressMap`, strat_table.c).
// ============================================================

/// Registry handles for the istrat entry points owned by this lane; the
/// table lane wires them onto the ISTRATS.ASM def_Istrat indices / the
/// synthetic address map (C `Strat_RegisterAll`).
pub struct EnemyBStratIds {
    /// `boss7_Istrat` (`Strat_Boss7_Init`, IS_BOSS7 = 99).
    pub boss7: StratId,
    /// `bossA_Istrat` (`Strat_BossA_Init`, IS_BOSSA = 85).
    pub bossa: StratId,
    /// `bossF_Istrat` (`Strat_BossF_Init`, IS_BOSSF = 116).
    pub bossf: StratId,
    /// `spacepilon_Istrat` (address-mapped at 0x030004).
    pub spacepilon: StratId,
    /// `tit_Istrat` (`Strat_Title_Init`, address-mapped at 0x050020).
    pub tit: StratId,
}

/// Register this lane's strategy entry points (idempotent — [`sid`]
/// memoizes on function identity) and return the public handles.
pub fn install_enemy_b(g: &mut Game) -> EnemyBStratIds {
    EnemyBStratIds {
        boss7: sid(g, strat_boss7_init),
        bossa: sid(g, strat_bossa_init),
        bossf: sid(g, strat_bossf_init),
        spacepilon: sid(g, strat_spacepilon_init),
        tit: sid(g, strat_title_init),
    }
}

/// C: the enemy_b-owned rows of `Strat_RegisterAll` /
/// `Strat_RegisterAddressMap` (strat_table.c). Populates the
/// `g_istrats[]` slots and the 24-bit strategy address map.
pub fn register(world: &mut sf_game::world::World) {
    fn world_sid(world: &mut sf_game::world::World, f: StrategyFn) -> StratId {
        if let Some(pos) = world
            .strat_registry
            .iter()
            .position(|&r| r as usize == f as usize)
        {
            return StratId(pos as u16);
        }
        world.register_strategy(f)
    }

    let boss7 = world_sid(world, strat_boss7_init);
    world.istrats[IS_BOSS7] = Some(boss7);
    let bossa = world_sid(world, strat_bossa_init);
    world.istrats[IS_BOSSA] = Some(bossa);
    let bossf = world_sid(world, strat_bossf_init);
    world.istrats[IS_BOSSF] = Some(bossf);

    let spacepilon = world_sid(world, strat_spacepilon_init);
    world.register_strategy_address(STRAT_ADDR_SPACEPILON, spacepilon);
    let tit = world_sid(world, strat_title_init);
    world.register_strategy_address(STRAT_ADDR_TIT, tit);
    world.register_strategy_address(STRAT_ADDR_BOSSF, bossf);
}
