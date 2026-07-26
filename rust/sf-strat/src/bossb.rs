//! Andross — the Venom final boss (RIIR port, ASM ground truth).
//!
//! Two ISTRAT entities from `reference/ultrastarfox/SF/STRAT/GB3STRAT.ASM`
//! (lines ~1252-3140), no C-oracle counterpart (`strat_boss*.c` never ported
//! Andross), so every cite below is to the 65816 source:
//!
//!   - `bossB`    (face)  = def_Istrat 115 (ISTRATS.ASM:539, MACRO-counted;
//!     verified vs seamon=81/boss8=84/boss2=108/bossg=144). Placed by
//!     `MAP1_5.ASM:176` (`boss_b_1,bossb_Istrat`) -> ported sf-map
//!     `route1/level1_5.rs:253` via synthetic addr `STRAT_ADDR_BOSSB`
//!     (0x06000F). We register that address here so the map resolves it.
//!   - `bossBrob` (robot) = def_Istrat 118 (ISTRATS.ASM:542). Placed by
//!     `MAP1_6A.ASM:292` (`boss_b_1,bossBrob_Istrat`), transcribed by sf-map
//!     `route1/level1_6.rs`. The compact map strategy row resolves directly to
//!     this native initializer.
//!
//! RETAIL COVERAGE: init + HP bar (s_set_bossmaxHP /
//! s_add_bossHP accumulator), the face's approach + dodge attack (quadrant
//! move-table + laser/homing fire) + HP-threshold phase-down (spin -> spinend
//! terminal drain) + escape-on-death (GF_BOSSDEAD); the robot's approach +
//! split (spawns the shootable face/hand parts that carry the boss bar) + the
//! 8-state attack rotation (bossBrobnextstate) firing each state + the Ouch
//! damage-reaction knockback/counter-fire + death -> explode; face `bossB_cont`
//! image trail (`bossBent` / `bossBspinend` every-other-frame spawn); scream /
//! ouch latch on ROM `sflag5` (`ASF3_SFLAG5`, not image `sflag1`); exact
//! active-bit animation timing for the intro, morph, jump, pounce, separation,
//! kick, and landing states; linked split-part shutdown and damage smoke.
//! Morph chain `bossBrobchg`→`chg2/3/4`→`bossBrobstart` is ported; undead/die
//! leaves are public for ledger coverage (cutscene / alternate death paths).
//! All weapon IDs used by these states dispatch through their exact shared
//! constructors (RELSLOWELASER[HOME], HMISSILE1, BOSSHMISSILE1,
//! CHICKHMISSILE1 and HPLASMA), including mesh, AP, lifetime, homing mode,
//! target pointer, muzzle transform and positional sound family.

#![allow(dead_code)]

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE3, ACF_COLLTYPE4, AFONFIRE, ASF_COLLDISABLE, ASF_COLLIDE,
    ASF_HITFLASH, ASF_NOHITAFFECT, ASF_SHADOW, ATGND, ATZREMOVE, NUMBER_AL,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::vars::GF_BOSSDEAD;
use sf_game::world::World;

use crate::common::sf_random;
use crate::common::{
    makesmoke_srou, strat_angle_xz as angle_xz, strat_apply_velocity as apply_velocity,
    strat_gen_vecs_3d as gen_vecs_3d, strat_gen_vecs_nvecs as gen_vecs_yaw,
    strat_make_obj as make_obj, strat_speed_to as speed_to,
};
use crate::enemy_a::{
    achase_angle, add_player_z, addrnd2pos_xy, boss_keeprel_to_player, copy_pos,
    fire_boss_hmissile1, fire_chick_hmissile1, fire_hmissile1, fire_hplasma, make_fol_exp_obj,
    make_large_exp_obj, make_medium_exp_obj, player, start_boss_explosion_circle, strat_aim_3d,
    strat_fire_relslowlaser, strat_fire_relslowlaserhome, strat_hit_flash, strat_pitch_toward,
    strat_qboss_explode_init, ASF4_NOPOLYEXP,
};

// ============================================================
// Constants (verbatim STRATEQU.INC / VARS.INC equs).
// ============================================================
const BOSSB_AIR_HP: u8 = 40; // STRATEQU.INC:284 bossBairHP
const BOSSB_SPIN_HP: u8 = 30; // STRATEQU.INC:285 bossBspinHP
const BOSSBROB_HP: u8 = 32; // STRATEQU.INC:286 bossBrobHP
const BOSSB_AP: u8 = 16; // STRATEQU.INC:287 bossBAP
const BOSSB_SCALE: u32 = 2; // STRATEQU.INC:302 bossB_scale
const HARDHP: u8 = 0xFF; // STRATEQU.INC:68 hardHP == -1

const ANIMATION_ACTIVE: u8 = 128;
const ANIMATION_FRAME_MASK: u8 = 127;
const BOSSBROB_DAMAGE_SMOKE_HP: u8 = 60;
const BOSSBROB_DAMAGE_SMOKE_PERIOD_BITS: u16 = 2;
const BOSSBROB_HIT_SOUND: u8 = 39;
const BOSSBROB_TOP_HIT_SOUND: u8 = 128;
const BOSSBROB_OUCH_DURATION: u8 = 16;
const BOSSBROB_OUCH_TABLE_STRIDE: u8 = 32;
const BOSSBROB_LEFT_OUCH_OFFSET: u8 = 0;
const BOSSBROB_RIGHT_OUCH_OFFSET: u8 = BOSSBROB_OUCH_TABLE_STRIDE;
const BOSSBROB_TOP_OUCH_OFFSET: u8 = 2 * BOSSBROB_OUCH_TABLE_STRIDE;
const BOSSBENT_SPLIT_DISSOLVE_THRESHOLD: u8 = 20;
const BOSSBROB_IDLE_FRAME: u8 = 0;
const BOSSBROB_CROUCH_FRAME: u8 = 12;
const BOSSBROB_KICK_FRAME: u8 = 15;
const BOSSBROB_FINAL_FRAME: u8 = 19;
const BOSSBROB_ANIMATION_FRAMES: u8 = 20;
const BOSSBROB_FORM_ANIMATION_FRAMES: u8 = 13;
const BOSSBROB_JUMP_SOUND: u8 = 77;
const BOSSBROB_LAND_SOUND: u8 = 76;
const BOSSBROB_MOVE_SOUND: u8 = 45;
const BOSSB_IMAGE_SOUND: u8 = 43;
const BOSSB_TRANSFORM_SOUND: u8 = 129;
const BOSSBROB_APPROACH_SOUND: u8 = 132;
const BOSSBROB_TRANSFORM_MUSIC: u8 = 241;
const BOSSBROB_DEATH_MUSIC: u8 = 240;
const BOSSBROB_FALL_BOUNCE_SHIFT: u32 = 2;
const BOSSBROB_FALL_GRAVITY: i16 = 2;
const BOSSBROB_SETTLED_BOUNCE_MIN: i16 = -5;
const BOSSBROB_DEATH_SCROLL_DISTANCE: i32 = 1300;

const DEG180: u8 = 128; // VARS.INC:12
const DEG90: u8 = 64; // VARS.INC:13
const DEG45: u8 = 32; // VARS.INC:14
const DEG11: u8 = 8; // VARS.INC:16 deg11 = deg360/32

const SPACE_VIEWCY: i16 = -60; // STRATEQU.INC:494

fn gsvar_byte1(g: &Game) -> u8 {
    g.vars.map.global_strategy_byte
}
fn set_gsvar_byte1(g: &mut Game, v: u8) {
    g.vars.map.global_strategy_byte = v;
}

// al_sflags2 bits (STRATEQU.INC:896-915 byte2 layout).
const ASF2_SFLAG1: u8 = 0x10; // "image 1"
const ASF2_SFLAG2: u8 = 0x20; // "image 2"
const ASF2_SFLAG3: u8 = 0x40;
const ASF2_SFLAG4: u8 = 0x80;
// al_sflags3 bit0 = ROM `sflag5` (after sflag1–4 in sflags2).
const ASF3_SFLAG5: u8 = 0x01;

// al_hitflags zone bits (VARS.INC:167-169 HF1..HF3).
const HF1: u8 = 1; // top
const HF2: u8 = 2; // left
const HF3: u8 = 4; // right

// enemy collision-type bits (COLLTYPE_*).
const COLLTYPE_ENEMY2: u8 = ACF_COLLTYPE3;
const COLLTYPE_ENEMYWEAP: u8 = ACF_COLLTYPE4;

/// `boss_b_1` from the canonical ISTRATS/shape compiler catalog.
const SH_BOSS_B_1: u16 = 76;
/// Walking-form animation meshes selected directly by GB3STRAT.
const SH_BOSS_B_0: u16 = 75;
const SH_BOSS_B_6: u16 = 468;
const SH_BOSS_B_7: u16 = 469;

/// STRAT_ADDR_BOSSB — the synthetic strategy address MAP1_5 uses for the face
/// (sf-map `route1/level1_5.rs:53`). Registered in `register()`.
pub const STRAT_ADDR_BOSSB: u32 = 0x06000F;

/// ISTRATS.ASM def_Istrat indices (MACRO-counted).
pub const IS_BOSSB: usize = 114;
pub const IS_BOSSBROB: usize = 117;

// ============================================================
// bossbpos_tab — the face's dodge move-table (GB3STRAT.ASM:1411-1430).
// 4 quadrant blocks (offsets 0,-400 / -400,0 / 400,0 / 0,400), each 8 entries:
// 4 at Move_perc 140 then 4 at 70, over space corners (half-space) << scale.
// Precomputed here (ASM integer division truncates toward zero).
// ============================================================
const BOSSB_POS_BASE: [(i16, i16); 8] = [
    // perc 140: (minx,miny)(maxx,miny)(minx,maxy)(maxx,maxy)
    (-672, -532),
    (672, -532),
    (-672, 224),
    (672, 224),
    // perc 70
    (-336, -264),
    (336, -264),
    (-336, 112),
    (336, 112),
];
const BOSSB_POS_QUAD: [(i16, i16); 4] = [(0, -400), (-400, 0), (400, 0), (0, 400)];

/// bossbpos_tab[index] (index 0..31): base entry + quadrant offset.
fn bossbpos_tab(index: u8) -> (i16, i16) {
    let i = (index & 0x1f) as usize;
    let base = BOSSB_POS_BASE[i & 7];
    let quad = BOSSB_POS_QUAD[(i >> 3) & 3];
    (base.0.wrapping_add(quad.0), base.1.wrapping_add(quad.1))
}

// ============================================================
// Shared helpers (STRATMAC semantics; ASM-cited).
// ============================================================

/// `s_jmp_notdelay N` (STRATMAC.INC:6456): TRUE when `gameframe & ((1<<N)-1)==0`.
fn notdelay(g: &Game, bits: u16) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}
/// `s_jmp_notdelay N,al1pt` per-object stagger (al1pt -> obj index, port cvt).
fn notdelay_stag(g: &Game, idx: u16, bits: u16) -> bool {
    g.vars.gameframe.wrapping_add(idx) & ((1u16 << bits) - 1) == 0
}

/// `s_jmp_Zdistmore x,y,#d` — TRUE when `|dz| >= d` (inclusive).
fn zdist_more(g: &Game, idx: u16, d: i16) -> bool {
    match player(g) {
        Some(p) => (p.worldz as i32 - g.objs.aliens[idx as usize].worldz as i32).abs() >= d as i32,
        None => false,
    }
}
/// `s_jmp_Zdistless x,y,#d` — TRUE when `|dz| < d` (strict).
fn zdist_less(g: &Game, idx: u16, d: i16) -> bool {
    match player(g) {
        Some(p) => (p.worldz as i32 - g.objs.aliens[idx as usize].worldz as i32).abs() < d as i32,
        None => false,
    }
}

/// `s_jmp_alvarINLIMIT W,x,al_worldx,L` — TRUE when `-L <= worldx <= L`.
fn worldx_in_limit(al: &Alien, l: i16) -> bool {
    al.worldx >= -l && al.worldx <= l
}

/// `s_achase_alvar W,...,rate` (STRATMAC.INC sr16 achase): 16-bit chase with a
/// nolessrange pre-clamp (min |step| = 1<<shift) then arithmetic `d >> shift`.
/// Twin of enemies_ground::achase_word / mother::achase16.
fn achase16(cur: i16, target: i16, shift: u32) -> i16 {
    let mut d = target as i32 - cur as i32;
    if d == 0 {
        return cur;
    }
    let min = 1i32 << shift;
    if d > -min && d < min {
        d = if d < 0 { -min } else { min };
    }
    cur.wrapping_add((d >> shift) as i16)
}

/// Authored animation frame, without the source active marker.
fn animation_frame(al: &Alien) -> u8 {
    al.animframe & ANIMATION_FRAME_MASK
}

/// Retail `s_init_anim`: select an authored frame and keep animation active.
fn init_animation(al: &mut Alien, frame: u8) {
    al.animframe = ANIMATION_ACTIVE | (frame & ANIMATION_FRAME_MASK);
}

/// Retail three-argument animation advance: wrap within the authored range.
fn add_animation_wrap(al: &mut Alien, amount: i8, maxframes: u8) {
    let mut frame = animation_frame(al) as i16 + amount as i16;
    while frame < 0 {
        frame += maxframes as i16;
    }
    while frame >= maxframes as i16 {
        frame -= maxframes as i16;
    }
    init_animation(al, frame as u8);
}

/// Retail labelled animation advance: clamp at the final frame and branch.
fn add_animation_cap(al: &mut Alien, amount: i8, maxframes: u8) -> bool {
    let frame = animation_frame(al) as i16 + amount as i16;
    if frame >= maxframes as i16 {
        init_animation(al, maxframes - 1);
        true
    } else {
        init_animation(al, frame.max(0) as u8);
        false
    }
}

/// Retail falling-body update. Position integration happens in the caller,
/// after this gravity, ground, and bounce step.
fn fall_down_y_velocity(al: &mut Alien, bounce_shift: u32, gravity: i16, ground: i16) -> bool {
    al.vy = al.vy.wrapping_add(gravity);
    if al.worldy < ground {
        return false;
    }
    al.worldy = ground;
    let mut velocity = al.vy.wrapping_neg() >> bounce_shift;
    if (BOSSBROB_SETTLED_BOUNCE_MIN..=0).contains(&velocity) {
        velocity = 0;
    }
    al.vy = velocity;
    velocity == 0
}

fn linked_object_has_flag3(g: &Game, idx: u16) -> bool {
    let raw = g.objs.aliens[idx as usize].ptr;
    let Some(linked) = raw.checked_sub(1) else {
        return false;
    };
    (linked as usize) < NUMBER_AL
        && g.objs.aliens[linked as usize].active
        && g.objs.aliens[linked as usize].sflags2 & ASF2_SFLAG3 != 0
}

/// ROM `bossBrange_srou` (GB3STRAT.ASM:1820): 2D range via `xydiffs_abs_l`.
pub fn bossbrange_srou(worldx: i16, worldy: i16, tx: i16, ty: i16) -> i16 {
    crate::common::xy_diffs_abs(worldx, worldy, tx, ty)
}

/// ROM `bossBpointdir_srou` (GB3STRAT.ASM:1814) — aim roty/rotx at (tx,ty,worldz).
pub fn bossbpointdir_srou(g: &mut Game, idx: u16, tx: i16, ty: i16) {
    let me = g.objs.aliens[idx as usize];
    let mut target = me;
    target.worldx = tx;
    target.worldy = ty;
    // Keep same Z (ROM s_set_wp uses al_worldz).
    strat_aim_3d(g, idx, &target, 2);
}

/// ROM `bossflash_l` (WINDOWS.ASM:240) — dyingred cyan color-math flash.
pub fn bossflash_l(g: &mut Game) {
    g.hooks.boss_flash();
}

/// Alias of [`crate::common::xy_diffs_abs`] (ROM `xydiffs_abs_l` Manhattan).
fn range_xy(worldx: i16, worldy: i16, tx: i16, ty: i16) -> i16 {
    crate::common::xy_diffs_abs(worldx, worldy, tx, ty)
}

/// s_gen_3dvecs + s_add_vecs2pos (move forward along roty/rotx at al_vel).
fn move_3d(al: &mut Alien) {
    gen_vecs_3d(al);
    apply_velocity(al);
}

/// s_set_bossmaxHP #v (STRATLIB.INC).
fn set_bossmaxhp(g: &mut Game, v: u16) {
    g.vars.bossmaxhp = v;
}
/// s_add_bossHP x,al_hp (STRATLIB.INC:562): m_bossHP += al_hp, per-tick from
/// every living part (accumulator zeroed each frame in init_strats). HUD bar =
/// m_bossHP / m_bossmaxHP.
fn add_bosshp(g: &mut Game, idx: u16) {
    let hp = g.objs.aliens[idx as usize].hp as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(hp);
}

/// s_copy_rots y,x — copy the 3 rotation bytes.
fn copy_rots(g: &mut Game, dst: u16, src: u16) {
    let s = g.objs.aliens[src as usize];
    let d = &mut g.objs.aliens[dst as usize];
    d.rotx = s.rotx;
    d.roty = s.roty;
    d.rotz = s.rotz;
}

// -------- exact shared weapon dispatch

/// RELSLOWELASER at self, aimed pitch/yaw with a ±spread (s_weapon_rndrot m,m =
/// per-axis (rnd&(2m-1))-m; STRATMAC.INC). Straight enemy laser.
fn fire_relslowlaser(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    strat_fire_relslowlaser(g, idx, pitch, yaw);
}

fn set_shot_target_player(g: &mut Game, shot: u16) {
    if let Some(target) = g.objs.player().map(|_| 0u16) {
        let raw = target.wrapping_add(1);
        g.objs.aliens[shot as usize].ptr = raw;
        g.objs.aliens[shot as usize].fireobjptr = raw;
    }
}

/// `s_weapon_rots2obj` / `s_weapon_rot` only changes the weapon scratch
/// angles, not the firer. Temporarily substituting the firer's bytes lets the
/// shared constructor consume those exact scratch values, then restores the
/// boss before returning.
fn fire_aimed_with(
    g: &mut Game,
    idx: u16,
    pitch: u8,
    yaw: u8,
    fire: fn(&mut Game, u16) -> Option<u16>,
) -> Option<u16> {
    let (save_x, save_y) = {
        let al = &g.objs.aliens[idx as usize];
        (al.rotx, al.roty)
    };
    g.objs.aliens[idx as usize].rotx = pitch;
    g.objs.aliens[idx as usize].roty = yaw;
    let shot = fire(g, idx);
    g.objs.aliens[idx as usize].rotx = save_x;
    g.objs.aliens[idx as usize].roty = save_y;
    if let Some(shot) = shot {
        set_shot_target_player(g, shot);
    }
    shot
}

fn fire_hmissile1_aimed(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let _ = fire_aimed_with(g, idx, pitch, yaw, fire_hmissile1);
}

fn fire_chick_hmissile1_aimed(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let _ = fire_aimed_with(g, idx, pitch, yaw, fire_chick_hmissile1);
}

fn fire_boss_hmissile1_aimed(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let _ = fire_aimed_with(g, idx, pitch, yaw, fire_boss_hmissile1);
}

fn fire_hplasma_aimed(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    if let Some(shot) = fire_aimed_with(g, idx, pitch, yaw, fire_hplasma) {
        // Every Andross HPLASMA site sets `s_weapon_pos #0,#0,#0` before
        // firing, so it starts at the boss rather than the shared scratch
        // muzzle used by other weapon families.
        copy_pos(g, shot, idx);
    }
}

/// Aim yaw+pitch at the player then fire a randomly-spread laser (the common
/// `.fire` tail shared by dodge / fireP1 / rndpos). Returns after firing.
fn aim_and_fire_laser(g: &mut Game, idx: u16, spread: u8) {
    if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let yaw = angle_xz(&me, &p);
        let pitch = strat_pitch_toward(&me, &p);
        let m = spread.max(1);
        let dyaw = (sf_random(&mut g.vars) as u8 % (2 * m + 1)).wrapping_sub(m);
        let dp = (sf_random(&mut g.vars) as u8 % (2 * m + 1)).wrapping_sub(m);
        fire_relslowlaser(g, idx, pitch.wrapping_add(dp), yaw.wrapping_add(dyaw));
    }
}

// ============================================================
// registry-id resolver (identical form to enemies_ground::wsid).
// ============================================================
fn sid(g: &mut Game, f: StrategyFn) -> StratId {
    if let Some(pos) = g
        .world
        .strat_registry
        .iter()
        .position(|&r| r as usize == f as usize)
    {
        StratId(pos as u16)
    } else {
        g.world.register_strategy(f)
    }
}
fn wsid(world: &mut World, f: StrategyFn) -> StratId {
    if let Some(pos) = world
        .strat_registry
        .iter()
        .position(|&r| r as usize == f as usize)
    {
        StratId(pos as u16)
    } else {
        world.register_strategy(f)
    }
}

// ############################################################
// bossB — the flying face (GB3STRAT.ASM:1252-1830).
// ############################################################

/// ROM `bossB_Istrat` (GB3STRAT.ASM:1252-1268).
pub fn bossb_istrat(g: &mut Game, idx: u16) {
    bossb_init(g, idx);
}

/// bossB_Istrat (GB3STRAT.ASM:1252-1268): wire data/ptrs, pose, then fall into
/// bossB_strat the same tick (no s_end between the init block and the label).
pub fn bossb_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossb_strat);
    let coll = sid(g, bossbdodgecol_istrat); // collstrat = hitflashBOSSd (Ouch flash)
    let exp = sid(g, bossbescape_istrat); // expstrat = bossBescape (death = flee)
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = BOSSB_AIR_HP; // s_set_aldata #bossBairHP,#bossBAP
        al.ap = BOSSB_AP;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
        al.rotx = DEG90.wrapping_neg(); // s_set_alvar al_rotx,#-deg90
        al.roty = DEG90; // s_set_alvar al_roty,#deg90
        al.vel = 40; // s_set_speed #40
        al.sflags2 |= ASF2_SFLAG1; // image 1
        al.sflags2 &= !ASF2_SFLAG2; // image 2 off
        al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag nohitaffect
    }
    set_bossmaxhp(g, BOSSB_AIR_HP as u16); // s_set_bossmaxHP #bossBairHP
    bossb_strat(g, idx);
}

/// ROM `bossB_strat` (GB3STRAT.ASM:1270-1309): face the player; hold at 2500 z, then
/// s_speedto 0 -> bossBdodge_init once stopped. `bossB_cont` spawns the image trail.
pub fn bossb_strat(g: &mut Game, idx: u16) {
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 4); // s_obj2obj_3dangle roty,rotx,4
    }
    if zdist_more(g, idx, 2500) {
        // .nstop -> bossB_cont3: keep flying forward.
        bossb_cont3(g, idx);
    } else {
        // s_speedto x,#0,1,bossBdodge_init — decelerate; switch when stopped.
        let stopped = speed_to(&mut g.objs.aliens[idx as usize], 0, 1);
        if stopped {
            bossbdodge_init(g, idx);
            return;
        }
        bossb_cont3(g, idx);
    }
}

/// ROM `bossB_cont3` (GB3STRAT.ASM:1279) — genvecs + addvecs then cont.
pub fn bossb_cont3(g: &mut Game, idx: u16) {
    move_3d(&mut g.objs.aliens[idx as usize]);
    bossb_cont(g, idx);
}

/// ROM `bossB_cont` (GB3STRAT.ASM:1283-1301) — every-other-frame image trail
/// (`bossBent` normally; `bossBspinend` when sflag2), then cont2.
pub fn bossb_cont(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        // s_not_alsflag sflag4; spawn only when clear after the toggle.
        g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG4;
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG4 == 0 {
            if let Some(trail) = make_obj(g, SH_BOSS_B_1) {
                // s_set_alvartobeobj y,al_ptr,x — raw mother index (matches
                // bossbspinend_strat's ptr read).
                g.objs.aliens[trail as usize].ptr = idx;
                if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 != 0 {
                    // Spinend trail: copy/inc mother's sword1 high byte & 3.
                    let hi = ((g.objs.aliens[idx as usize].sword1 as u16) >> 8) as u8;
                    g.objs.aliens[trail as usize].sword1 = ((hi as u16) << 8) as i16;
                    let nhi = hi.wrapping_add(1) & 3;
                    let lo = g.objs.aliens[idx as usize].sword1 as u16 & 0xff;
                    g.objs.aliens[idx as usize].sword1 = (lo | ((nhi as u16) << 8)) as i16;
                    bossbspinend_istrat(g, trail);
                } else {
                    bossbent_istrat(g, trail);
                }
                copy_pos(g, trail, idx);
                copy_rots(g, trail, idx);
            }
        }
    }
    bossb_cont2(g, idx);
}

/// ROM `bossB_cont2` (GB3STRAT.ASM:1302) — add_bossHP then cont4.
pub fn bossb_cont2(g: &mut Game, idx: u16) {
    add_bosshp(g, idx);
    bossb_cont4(g, idx);
}

/// ROM `bossB_cont4` / `bossBaddpz_cont` (GB3STRAT.ASM:1304 / 2197).
pub fn bossb_cont4(g: &mut Game, idx: u16) {
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBaddpz_cont` (GB3STRAT.ASM:2197) — add_playerZ then addbhp.
pub fn bossbaddpz_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    bossbaddbhp_cont(g, idx);
}

/// ROM `bossBaddbhp_cont` (GB3STRAT.ASM:2199).
pub fn bossbaddbhp_cont(g: &mut Game, idx: u16) {
    add_bosshp(g, idx);
}

/// ROM `bossBdodge_init` (GB3STRAT.ASM:1311-1316): enter the dodge attack.
pub fn bossbdodge_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbdodge_strat);
    let coll = sid(g, bossbdodgecol_istrat); // bossBdodgecol -> hitflashBOSSd
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte3 = 1; // s_set_alvar al_sbyte3,#1
        al.collstratptr = Some(coll);
        al.sflags &= !ASF_NOHITAFFECT; // s_clr_alsflag nohitaffect (now damageable)
    }
    bossbdodge_strat(g, idx);
}

/// ROM `bossBdodge_strat` (GB3STRAT.ASM:1316-1401): the face's main attack. Picks a
/// target quadrant from the player's screen position, chases the move-table
/// point, and fires (laser when facing / homing missile when repositioning).
/// At hp<16 it hands off to the spin wind-up.
pub fn bossbdodge_strat(g: &mut Game, idx: u16) {
    // s_jmp_alvarless al_hp,#16,bossBspin1_init
    if g.objs.aliens[idx as usize].hp < 16 {
        bossbspin1_init(g, idx);
        return;
    }
    // s_decbne_alvar al_sbyte3,.same — retarget only when the timer expires.
    let expired = {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte3 != 0 {
            al.sbyte3 -= 1;
        }
        al.sbyte3 == 0
    };
    if expired {
        g.objs.aliens[idx as usize].sbyte3 = 1;
        // Quadrant from player screen pos: right?+1, up?+2, then <<2 (GB3:1329).
        let mut q: u8 = 0;
        if let Some(p) = player(g) {
            if (p.worldx >> 8) >= 0 {
                q += 1; // player_posx+1 bpl -> right
            }
            if p.worldy.wrapping_sub(SPACE_VIEWCY) >= 0 {
                q += 2; // above center
            }
        }
        q <<= 2;
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 != q {
            al.sbyte1 = q; // new base index
            al.sbyte2 = 0;
            // s_jmp_random .same,80 ; s_set_alvar al_sbyte2,#16 — 80/255 chance
            // of the +16 (second half-space) variant.
            if (sf_random(&mut g.vars) as u8) >= 127 {
                g.objs.aliens[idx as usize].sbyte2 = 16;
            }
        }
    }
    // svar_byte2 = sbyte1 + sbyte2 -> move-table index.
    let index = g.objs.aliens[idx as usize]
        .sbyte1
        .wrapping_add(g.objs.aliens[idx as usize].sbyte2);
    let (tx, ty) = bossbpos_tab(index);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, tx, 3);
        al.worldy = achase16(al.worldy, ty, 3);
    }
    // bossBrange_srou + s_jmp_varless rangexz,#300,.faceplayer.
    let me = g.objs.aliens[idx as usize];
    let range = bossbrange_srou(me.worldx, me.worldy, tx, ty);
    if range < 300 {
        // .faceplayer: aim tight (shift 2) then fire on the notdelay 1 gate.
        if let Some(p) = player(g) {
            g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
            strat_aim_3d(g, idx, &p, 2);
        }
        if notdelay(g, 1) {
            aim_and_fire_laser(g, idx, 3); // s_weapon_rndrot 3,3
        }
    } else {
        // .nthere: mark repositioning, point at the target, lob a homing
        // missile on the notdelay 3 gate.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags2 |= ASF2_SFLAG1;
            al.sbyte3 = 25;
        }
        bossbpointdir_srou(g, idx, tx, ty);
        if notdelay(g, 3) {
            if let Some(p) = player(g) {
                let m = g.objs.aliens[idx as usize];
                let yaw = angle_xz(&m, &p);
                let pitch = strat_pitch_toward(&m, &p);
                fire_hmissile1_aimed(g, idx, pitch, yaw);
            }
        }
    }
    bossb_cont(g, idx);
}

/// ROM `bossBdodgecol_Istrat` (GB3STRAT.ASM:1404) — reset retarget timer + hitflashBOSSd.
pub fn bossbdodgecol_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte3 = 1;
    crate::enemy_a::hitflash_bossd_istrat(g, idx);
}

/// ROM `bossBspin1_init` / `bossBspin1_strat` (GB3STRAT.ASM:1436).
pub fn bossbspin1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspin1_strat);
    let coll = sid(g, strat_hit_flash);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags2 |= ASF2_SFLAG1;
    al.collstratptr = Some(coll);
    bossbspin1_strat(g, idx);
}
pub fn bossbspin1_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, 0, 3);
        al.worldy = achase16(al.worldy, SPACE_VIEWCY + 100, 3);
        achase_angle(&mut al.roty, DEG90, 2);
        achase_angle(&mut al.rotx, 0, 2);
    }
    let me = g.objs.aliens[idx as usize];
    if range_xy(me.worldx, me.worldy, 0, SPACE_VIEWCY + 100) < 300 {
        bossbspin2_init(g, idx);
        return;
    }
    bossb_cont2(g, idx);
}

/// ROM `bossBspin2_init` / `bossBspin2_strat` (GB3STRAT.ASM:1463).
pub fn bossbspin2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspin2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte3 = 0;
    al.roty = DEG90;
    al.rotx = 0;
    al.sflags |= ASF_NOHITAFFECT;
    al.sflags2 &= !ASF2_SFLAG4;
    bossbspin2_strat(g, idx);
}
pub fn bossbspin2_strat(g: &mut Game, idx: u16) {
    let at_speed = speed_to(&mut g.objs.aliens[idx as usize], 120, 2);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if at_speed {
            al.sflags2 |= ASF2_SFLAG2;
        } else {
            al.sbyte1 = 5;
        }
        al.rotx = al.rotx.wrapping_sub(16); // spin the pitch
        gen_vecs_3d(al);
        apply_velocity(al);
    }
    // s_beqdec_alvar al_sbyte1,bossBspinend2_init
    let al = &mut g.objs.aliens[idx as usize];
    if al.sbyte1 == 0 {
        bossbspinend2_init(g, idx);
        return;
    }
    al.sbyte1 -= 1;
    bossb_cont2(g, idx);
}

/// ROM `bossBspinend2_init` (GB3STRAT.ASM:1495) — terminal drain + scream gate.
pub fn bossbspinend2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspinend2_strat);
    let coll = sid(g, bossbspinendcol_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = BOSSB_SPIN_HP;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.sflags &= !ASF_NOHITAFFECT;
        al.sbyte4 = 0;
        al.sword2 = al.hp as i16;
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.ptr = idx; // self-ptr for child image tracking
    }
    set_gsvar_byte1(g, 0);
    set_bossmaxhp(g, BOSSB_SPIN_HP as u16);
    // ROM falls into bossBspinend_Icont then jmpto_strat of spinend2.
    g.objs.aliens[idx as usize].sbyte3 = 1;
    bossbspinend2_strat(g, idx);
}

/// ROM `bossBspinend2_strat` — scream on big HP drop; else fire + cont.
pub fn bossbspinend2_strat(g: &mut Game, idx: u16) {
    let dmg = {
        let al = &g.objs.aliens[idx as usize];
        let d = (al.sword2 as i16) - (al.hp as i16);
        d.abs()
    };
    if dmg >= 6 {
        bossbscream_istrat(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sword2 = g.objs.aliens[idx as usize].hp as i16;

    if gsvar_byte1(g) == 2 {
        set_gsvar_byte1(g, 0);
        bossbspinendnewent_srou(g, idx);
    }

    // s_decbne_alvar al_sbyte3 — retarget timer for image spawn cadence.
    let expired = {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte3 != 0 {
            al.sbyte3 -= 1;
        }
        al.sbyte3 == 0
    };
    if expired {
        g.hooks.play_se(BOSSB_IMAGE_SOUND);
        let mut period: u8 = 30;
        if gsvar_byte1(g) == 2 {
            period = 15;
        }
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte3 = period;
            al.sbyte1 = (sf_random(&mut g.vars) as u8 & 7) << 2;
        }
    }

    if g.objs.aliens[idx as usize].sbyte4 == 0 {
        bossbspinend_cont(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte4 -= 1;
    if g.objs.aliens[idx as usize].sbyte4 <= 3 {
        g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG3;
        if g.objs.aliens[idx as usize].sbyte4 != 3 {
            bossb_cont2(g, idx);
            return;
        }
        g.objs.aliens[idx as usize].sbyte3 = 1;
        bossb_cont2(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG3;
    bossbspinend_cont(g, idx);
}

/// ROM `bossBspinendnewent_srou` (GB3STRAT.ASM:1720).
pub fn bossbspinendnewent_srou(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.sbyte4 = 14;
    al.sflags2 &= !ASF2_SFLAG4;
    al.sbyte3 = 1;
    // s_set_alvar2rnd x,al_sword1+1,#3 — high byte slot 0..3.
    let hi = (sf_random(&mut g.vars) as u8) & 3;
    al.sword1 = ((hi as u16) << 8) as i16;
}

/// ROM `bossBspinendcol_Istrat` (GB3STRAT.ASM:1706) — hitflashBOSSd.
pub fn bossbspinendcol_istrat(g: &mut Game, idx: u16) {
    strat_hit_flash(g, idx);
}

/// ROM `bossBspinendentcol_Istrat` (GB3STRAT.ASM:1699) — bump gsvar, become trail.
pub fn bossbspinendentcol_istrat(g: &mut Game, idx: u16) {
    set_gsvar_byte1(g, gsvar_byte1(g).wrapping_add(1));
    bossbent_istrat(g, idx);
}

/// ROM `bossBspinend_Istrat` (GB3STRAT.ASM:1615) — child image during spinend.
pub fn bossbspinend_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspinend_strat);
    let coll = sid(g, bossbspinendentcol_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.hp = HARDHP;
        al.ap = 0;
        al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
    }
    bossbspinend_icont(g, idx);
}

/// ROM `bossBspinend_Icont`.
pub fn bossbspinend_icont(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte3 = 1;
    bossbspinend_strat(g, idx);
}

/// ROM `bossBspinend_strat` — follow mother; die into bossBent when mother sflag3.
pub fn bossbspinend_strat(g: &mut Game, idx: u16) {
    let mother = g.objs.aliens[idx as usize].ptr;
    if mother != 0 && mother < g.objs.aliens.len() as u16 && g.objs.aliens[mother as usize].active {
        if g.objs.aliens[mother as usize].sflags2 & ASF2_SFLAG3 != 0 {
            bossbent_istrat(g, idx);
            return;
        }
    }
    bossbspinend_cont(g, idx);
}

/// ROM `bossBspinend_cont` (GB3STRAT.ASM:1645) — chase bossbpos_tab + fire.
pub fn bossbspinend_cont(g: &mut Game, idx: u16) {
    let mother = g.objs.aliens[idx as usize].ptr;
    let base = if mother != 0 && mother < g.objs.aliens.len() as u16 {
        g.objs.aliens[mother as usize].sbyte1
    } else {
        g.objs.aliens[idx as usize].sbyte1
    };
    let slot = ((g.objs.aliens[idx as usize].sword1 as u16) >> 8) as u8 & 3;
    let index = base.wrapping_add(slot << 5);
    let (tx, ty) = bossbpos_tab(index);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, tx, 3);
        al.worldy = achase16(al.worldy, ty, 3);
    }
    let me = g.objs.aliens[idx as usize];
    let range = range_xy(me.worldx, me.worldy, tx, ty);
    if range < 300 {
        // ROM `.faceplayer`: RELSLOWELASERHOME + lasersound (not bare RELSLOW).
        if let Some(p) = player(g) {
            strat_aim_3d(g, idx, &p, 1);
            if notdelay_stag(g, idx, 2) {
                let m = g.objs.aliens[idx as usize];
                let yaw = angle_xz(&m, &p);
                let pitch = strat_pitch_toward(&m, &p);
                // s_weapon_rndrot 7,7
                let dp = ((sf_random(&mut g.vars) as u8 & 7) as i16 - 3) as u8;
                let dy = ((sf_random(&mut g.vars) as u8 & 7) as i16 - 3) as u8;
                strat_fire_relslowlaserhome(g, idx, pitch.wrapping_add(dp), yaw.wrapping_add(dy));
            }
        }
    } else if notdelay_stag(g, idx, 3) {
        // ROM far path: HMISSILE1 + missilesound.
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            fire_hmissile1_aimed(g, idx, pitch, yaw);
        }
    }
    bossb_cont2(g, idx);
}

/// ROM `bossBscream_Istrat` / `strat` (GB3STRAT.ASM:1557).
pub fn bossbscream_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbscream_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 30;
        al.sflags |= ASF_NOHITAFFECT;
        al.sflags3 |= ASF3_SFLAG5; // ROM sflag5 (not image sflag1)
    }
    bossbscream_strat(g, idx);
}

pub fn bossbscream_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbscream2_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.roty, DEG180, 2);
        achase_angle(&mut al.rotx, 0, 2);
        al.worldx = achase16(al.worldx, 0, 2);
        al.worldy = achase16(al.worldy, SPACE_VIEWCY + 100, 2);
        al.sflags2 |= ASF2_SFLAG3;
    }
    bossb_cont2(g, idx);
}

/// ROM `bossBscream2_init` / `strat` (GB3STRAT.ASM:1579).
pub fn bossbscream2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbscream2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    g.objs.aliens[idx as usize].sbyte1 = 93;
    bossbscream2_strat(g, idx);
}

pub fn bossbscream2_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbscreamend_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    if g.objs.aliens[idx as usize].sbyte1 == 66 {
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            fire_chick_hmissile1_aimed(g, idx, pitch, yaw);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG180;
        let rnd = sf_random(&mut g.vars) as u8 & 15;
        al.roty = al.roty.wrapping_add(rnd).wrapping_sub(7);
    }
    bossb_cont2(g, idx);
}

/// ROM `bossBscreamend_init` (GB3STRAT.ASM:1603) — back to spinend2.
pub fn bossbscreamend_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspinend2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte4 = 14;
        al.sbyte3 = 1;
        al.sflags2 &= !ASF2_SFLAG4;
        al.sflags &= !ASF_NOHITAFFECT;
    }
    bossbspinend2_strat(g, idx);
}

/// ROM `bossBescape_Istrat` (GB3STRAT.ASM:1728).
pub fn bossbescape_istrat(g: &mut Game, idx: u16) {
    bossb_escape_init(g, idx);
}

/// ROM `bossBescape_strat` (GB3STRAT.ASM:1736).
pub fn bossbescape_strat(g: &mut Game, idx: u16) {
    bossb_escape_strat(g, idx);
}

/// bossBescape_Istrat/strat (GB3STRAT.ASM:1728-1770): the face's death — set
/// GF_BOSSDEAD, climb, accelerate to 100, and fly off. This IS bossB's defeat
/// (the ROM face flees rather than exploding; the robot form is the true kill).
pub fn bossb_escape_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossb_escape_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.expstratptr = Some(tick);
        al.stratptr = Some(tick);
        al.vel = 0; // s_set_speed #0
        al.sflags2 |= ASF2_SFLAG3;
        al.count = 60; // s_set_lifecnt #60
    }
    g.vars.gameflags |= GF_BOSSDEAD; // s_or_var gameflags,#gf_bossdead
    g.hooks.play_se(BOSSB_TRANSFORM_SOUND);
    bossb_escape_strat(g, idx);
}
fn bossb_escape_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 100, 1);
    if zdist_less(g, idx, 3000) && g.objs.aliens[idx as usize].rotx != DEG90 {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(1); // pitch up to flee
        al.count = al.count.saturating_sub(1);
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.roty, 0, 4);
        achase_angle(&mut al.rotx, 0, 3);
    }
    move_3d(&mut g.objs.aliens[idx as usize]);
    bossb_cont4(g, idx);
}

// ############################################################
// bossBrob — the robot (GB3STRAT.ASM:1877-3140).
// ############################################################

/// bossBrob_Istrat (GB3STRAT.ASM:1877-1912): wire data/ptrs, pose facing away,
/// then fall into bossBrob_strat. expstrat is the death->explode (ROM chains
/// bossBrobchg's multi-form transform; ported terminal explodes — module doc).
pub fn bossbrob_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_strat);
    let coll = sid(g, bossbrobcol_istrat); // hitflashbossD (Ouch) via collstrat
    let exp = sid(g, bossbrobchg_istrat); // ROM: bossBrobchg_Istrat
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = BOSSBROB_HP; // s_set_aldata #bossBrobHP,#bossBAP
        al.ap = BOSSB_AP;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
        al.vel = 40; // s_set_speed #40
        al.sflags2 |= ASF2_SFLAG1;
        al.sflags2 &= !ASF2_SFLAG2;
        al.sflags |= ASF_NOHITAFFECT | ASF_SHADOW;
        al.roty = DEG180.wrapping_sub(DEG45); // face away-left
        al.rotx = DEG11;
        al.sbyte1 = DEG180.wrapping_sub(DEG45); // turn latch
    }
    set_bossmaxhp(g, BOSSBROB_HP as u16); // s_set_bossmaxHP #bossBrobHP
    bossbrob_strat(g, idx);
}

/// ROM `bossBrob_strat` (GB3STRAT.ASM:1946).
pub fn bossbrob_strat(g: &mut Game, idx: u16) {
    // s_jmp_notdelay 5,.nturn — flip the sway target on the 32-frame gate.
    if notdelay(g, 5) {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = if al.sbyte1 == DEG180.wrapping_add(DEG45) {
            DEG180.wrapping_sub(DEG45)
        } else {
            DEG180.wrapping_add(DEG45)
        };
    }
    let target = g.objs.aliens[idx as usize].sbyte1;
    achase_angle(&mut g.objs.aliens[idx as usize].roty, target, 2);

    if zdist_more(g, idx, 3000) {
        bossbrob_cont3(g, idx);
        return;
    }
    if worldx_in_limit(&g.objs.aliens[idx as usize], 200) {
        bossbrob2_init(g, idx);
        return;
    }
    bossbrob_cont3(g, idx);
}

/// bossB_cont3 tail (GB3STRAT.ASM:1279-1308 shared): move then bossB_cont2.
fn bossbrob_cont3(g: &mut Game, idx: u16) {
    move_3d(&mut g.objs.aliens[idx as usize]);
    add_bosshp(g, idx);
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrob2_init` / `bossBrob2_strat` (GB3STRAT.ASM:1971).
pub fn bossbrob2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    bossbrob2_strat(g, idx);
}

pub fn bossbrob2_strat(g: &mut Game, idx: u16) {
    if speed_to(&mut g.objs.aliens[idx as usize], 0, 1) {
        bossbrobsplit_init(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.roty, DEG180, 3);
        achase_angle(&mut al.rotx, 0, 3);
    }
    bossbrob_cont3(g, idx);
}

/// ROM `bossBrobcent_srou` (GB3STRAT.ASM:2161): recenter to (0,-350).
pub fn bossbrobcent_srou(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, 0, 3);
        al.worldy = achase16(al.worldy, -350, 3);
    }
    if zdist_more(g, idx, 3000) {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = al.worldz.wrapping_add(60);
    }
}

/// ROM `bossBrobsplit_init` / `bossBrobsplit_strat` (GB3STRAT.ASM:1984).
pub fn bossbrobsplit_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobsplit_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags2 &= !ASF2_SFLAG3;
    bossbrobsplit_strat(g, idx);
}

pub fn bossbrobsplit_strat(g: &mut Game, idx: u16) {
    bossbrobcent_srou(g, idx);
    let me = g.objs.aliens[idx as usize];
    if range_xy(me.worldx, me.worldy, 0, -350) < 50 {
        bossbrobsplit2_init(g, idx);
        return;
    }
    add_bosshp(g, idx);
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrobsplit2_init` (GB3STRAT.ASM:2004): spawn the two other parts.
pub fn bossbrobsplit2_init(g: &mut Game, idx: u16) {
    // s_set_var2rnd svar_byte1,#3 ; reject 3 (0..2).
    let mut pick = sf_random(&mut g.vars) as u8 & 3;
    while pick == 3 {
        pick = sf_random(&mut g.vars) as u8 & 3;
    }
    // Part positions (al_sword1/sword2 rest targets), GB3STRAT.ASM:1985-1998.
    let parts = [(0i16, -400i16), (-450, -100), (450, -100)];
    for (i, &(sx, sy)) in parts.iter().enumerate() {
        if pick as usize == i {
            continue; // this is the body — set below.
        }
        if let Some(child) = bossbrob_ment(g, idx, (i + 1) as u8) {
            let al = &mut g.objs.aliens[child as usize];
            al.sword1 = sx;
            al.sword2 = sy;
        }
    }
    // The body becomes bossBentsplit2 (the aggressive central part).
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sword1 = parts[pick as usize].0;
        al.sword2 = parts[pick as usize].1;
    }
    bossbentsplit2_init(g, idx);
}

/// bossBrobMent_srou (GB3STRAT.ASM:2059-2069): spawn a duplicate part whose
/// typed object link names the source object. This is not a family-tree link;
/// the source keeps the mother's coordinate fields available to gameplay.
fn bossbrob_ment(g: &mut Game, mother: u16, _child_num: u8) -> Option<u16> {
    let source_shape = g.objs.aliens[mother as usize].shape;
    let child = make_obj(g, source_shape)?;
    copy_pos(g, child, mother);
    copy_rots(g, child, mother);
    let init = sid(g, bossbentsplit_istrat);
    {
        let part = &mut g.objs.aliens[child as usize];
        part.ptr = mother.wrapping_add(1);
        part.stratptr = Some(init);
    }
    Some(child)
}

/// ROM `bossBentsplit_Istrat` (GB3STRAT.ASM:2075).
pub fn bossbentsplit_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbentsplit_strat);
    let coll = sid(g, bossbentsplitcol_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARDHP;
    al.ap = 0;
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.depthoffset = 1;
    al.count = 8;
    al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
    bossbentsplit_icont(g, idx);
}

/// ROM `bossBentsplit_Icont`.
pub fn bossbentsplit_icont(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = 1;
    bossbentsplit_cont(g, idx);
}

/// ROM `bossBentsplitcol_Istrat` (GB3STRAT.ASM:2068).
pub fn bossbentsplitcol_istrat(g: &mut Game, idx: u16) {
    g.hooks.play_se(BOSSBROB_HIT_SOUND);
    g.objs.aliens[idx as usize].sflags &= !ASF_COLLIDE;
    if let Some(tick) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(tick, idx);
    }
}

/// ROM `bossBentsplit_strat` (GB3STRAT.ASM:2086).
pub fn bossbentsplit_strat(g: &mut Game, idx: u16) {
    if notdelay_stag(g, idx, 4) {
        aim_and_fire_laser(g, idx, 0);
    }
    if linked_object_has_flag3(g, idx)
        || g.objs.aliens[idx as usize].sbyte1 <= BOSSBENT_SPLIT_DISSOLVE_THRESHOLD
    {
        if notdelay(g, 3) {
            g.objs.aliens[idx as usize].depthoffset =
                g.objs.aliens[idx as usize].depthoffset.wrapping_add(1);
        }
        let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
        g.objs.aliens[idx as usize].count = c;
        if c == 0 {
            g.objs.aldead = 1;
            return;
        }
    }
    bossbentsplit_cont(g, idx);
}

/// ROM `bossBentsplit_cont` (GB3STRAT.ASM:2108) — drift / recenter.
pub fn bossbentsplit_cont(g: &mut Game, idx: u16) {
    // s_decbne_alvar al_sbyte1,.nres ; else #100
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = al.sbyte1.wrapping_sub(1);
        if al.sbyte1 == 0 {
            al.sbyte1 = 100;
        }
    }
    let sb1 = g.objs.aliens[idx as usize].sbyte1;
    if sb1 >= 20 {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, al.sword1, 3);
        al.worldy = achase16(al.worldy, al.sword2, 3);
        if zdist_more(g, idx, 1500) {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldz = al.worldz.wrapping_sub(60);
        }
    } else {
        bossbrobcent_srou(g, idx);
    }
    add_player_z(g, idx);
}

/// ROM `bossBentsplit2_Istrat` (GB3STRAT.ASM:2041).
pub fn bossbentsplit2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbentsplit2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags &= !ASF_NOHITAFFECT;
    bossbentsplit_icont(g, idx);
    // Icont sets sbyte1=1 then cont underflows→100 (GB3STRAT.ASM:2126).
    g.objs.aliens[idx as usize].sbyte1 = 100;
    bossbentsplit2_strat(g, idx);
}

/// Compatibility alias used by split2_init.
fn bossbentsplit2_init(g: &mut Game, idx: u16) {
    bossbentsplit2_istrat(g, idx);
}

/// ROM `bossBentsplit2_strat` (GB3STRAT.ASM:2047).
pub fn bossbentsplit2_strat(g: &mut Game, idx: u16) {
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 2);
        if notdelay_stag(g, idx, 3) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            // ROM RELSLOWELASERHOME → lasersound_l.
            strat_fire_relslowlaserhome(g, idx, pitch, yaw);
        }
    }
    add_bosshp(g, idx);
    if g.objs.aliens[idx as usize].sbyte1 == 1 {
        bossbrobsplit2_init(g, idx);
        return;
    }
    bossbentsplit_cont(g, idx);
    boss_keeprel_to_player(g, idx);
}

/// Compatibility alias for ment spawn.
fn bossbentsplit_init(g: &mut Game, idx: u16) {
    bossbentsplit_istrat(g, idx);
}

/// ROM `bossBrobcol_Istrat` (GB3STRAT.ASM:2949) — zone Ouch or flash.
pub fn bossbrobcol_istrat(g: &mut Game, idx: u16) {
    bossbrob_col(g, idx);
}

fn resume_after_bossbrob_collision(g: &mut Game, idx: u16, sound: u8) {
    g.hooks.play_se(sound);
    let tick = {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_COLLIDE;
        al.hitflags = 0;
        al.stratptr
    };
    if let Some(tick) = tick {
        g.call_strat(tick, idx);
    }
}

fn bossbrob_collision_reaction(g: &mut Game, idx: u16) {
    let partner = g.objs.aliens[idx as usize].collobjptr;
    if (partner as usize) < NUMBER_AL && g.objs.aliens[partner as usize].active {
        let _ = make_medium_exp_obj(g, partner);
    }
    let hf = g.objs.aliens[idx as usize].hitflags;
    let reaction = if hf & HF1 != 0 {
        Some((BOSSBROB_TOP_OUCH_OFFSET, BOSSBROB_TOP_HIT_SOUND, true))
    } else if hf & HF2 != 0 {
        Some((BOSSBROB_LEFT_OUCH_OFFSET, BOSSBROB_HIT_SOUND, false))
    } else if hf & HF3 != 0 {
        Some((BOSSBROB_RIGHT_OUCH_OFFSET, BOSSBROB_HIT_SOUND, false))
    } else {
        None
    };
    match reaction {
        Some((table_offset, sound, hit_flash)) => {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte4 = table_offset;
            al.sbyte3 = BOSSBROB_OUCH_DURATION;
            al.sflags3 |= ASF3_SFLAG5;
            if hit_flash {
                al.sflags |= ASF_HITFLASH;
            }
            resume_after_bossbrob_collision(g, idx, sound);
        }
        None => resume_after_bossbrob_collision(g, idx, BOSSBROB_HIT_SOUND),
    }
}

/// bossBrob_col — shared body for col / sepcol zone routing.
fn bossbrob_col(g: &mut Game, idx: u16) {
    let al = &g.objs.aliens[idx as usize];
    if al.sflags3 & ASF3_SFLAG5 != 0 || al.sflags & ASF_NOHITAFFECT != 0 {
        resume_after_bossbrob_collision(g, idx, BOSSBROB_HIT_SOUND);
        return;
    }
    bossbrob_collision_reaction(g, idx);
}

/// ROM `bossBrobsepcol_Istrat` (GB3STRAT.ASM:2821) — hits until pounce, else Ouch.
pub fn bossbrobsepcol_istrat(g: &mut Game, idx: u16) {
    let al = &g.objs.aliens[idx as usize];
    if al.sflags3 & ASF3_SFLAG5 != 0 || al.sflags & ASF_NOHITAFFECT != 0 {
        resume_after_bossbrob_collision(g, idx, BOSSBROB_HIT_SOUND);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte2 = al.sbyte2.wrapping_sub(1);
        if al.sbyte2 == 0 {
            al.sflags2 |= ASF2_SFLAG1;
            al.sflags &= !ASF_COLLIDE;
            al.hitflags = 0;
            bossbrob_nextstate(g, idx);
            return;
        }
    }
    bossbrob_collision_reaction(g, idx);
}

// ---- the 8-state attack rotation --------------------------------------------

/// bossBrobnextstate (GB3STRAT.ASM:3127-3138): advance the 1..8 state counter
/// and dispatch. States map to fireP1 / pouncepos / farjump / sep / fireP1 /
/// farjump / kick / farjump (default kick). This is the robot's attack loop.
fn bossbrob_nextstate(g: &mut Game, idx: u16) {
    // s_next_state x,#8 — cycle 1..=8.
    let s = {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratstate = al.stratstate.wrapping_add(1);
        if al.stratstate == 0 || al.stratstate > 8 {
            al.stratstate = 1;
        }
        al.stratstate
    };
    match s {
        1 | 5 => bossbrobfirep1_init(g, idx),
        2 => bossbrobpouncepos_init(g, idx),
        3 | 6 | 8 => bossbrobfarjump1_init(g, idx),
        4 => bossbrobsep_init(g, idx),
        7 => bossbrobkick_init(g, idx),
        _ => bossbrobkick_init(g, idx),
    }
}

/// ROM `bossbrobfireP1_init` (GB3STRAT.ASM:2415).
pub fn bossbrobfirep1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfirep1_strat);
    let coll = sid(g, bossbrobcol_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.sbyte1 = 60;
    bossbrobfirep1_strat(g, idx);
}

/// ROM `bossbrobfireP1_strat` (GB3STRAT.ASM:2419) — plant + HPLASMA for ~60 ticks.
pub fn bossbrobfirep1_strat(g: &mut Game, idx: u16) {
    bossbrobfrontplayer_srou(g, idx, 1400);
    // s_beqdec_alvar al_sbyte1,bossBrobstart2_init
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrobstart2_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    if notdelay(g, 4) {
        // `s_weapon_rot #0,#0`: launch along the boss's current orientation;
        // homingflat turns toward the explicit player target after spawn.
        let m = g.objs.aliens[idx as usize];
        fire_hplasma_aimed(g, idx, m.rotx, m.roty);
    }
    bossbrobouch_srou(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossbrobfire1_init` (GB3STRAT.ASM:2368) — save yaw, arm 60-tick spray.
pub fn bossbrobfire1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfire1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte2 = al.roty; // restore target for fire2
        al.sbyte1 = 60;
    }
    bossbrobfire1_strat(g, idx);
}

/// ROM `bossbrobfire1_strat` (GB3STRAT.ASM:2372) — rndrot laser + home laser.
pub fn bossbrobfire1_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrobfire2_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    if notdelay(g, 3) {
        // s_weapon_rndrot 3,15 — pitch mask 3, yaw mask 15
        if let Some(p) = player(g) {
            let me = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&me, &p);
            let pitch = strat_pitch_toward(&me, &p);
            let dp = ((sf_random(&mut g.vars) as u8) & 3).wrapping_sub(1);
            let dy = ((sf_random(&mut g.vars) as u8) & 15).wrapping_sub(7);
            strat_fire_relslowlaser(g, idx, pitch.wrapping_add(dp), yaw.wrapping_add(dy));
        }
    }
    // s_obj2obj_angle …,3,.fire2 — when yaw already on target, also fire home.
    if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        // ROM `s_obj2obj_angle` — Yanglexy+nega into body roty.
        let target = angle_xz(&me, &p).wrapping_neg();
        let mut yaw = me.roty;
        let _ = achase_angle(&mut yaw, target, 3);
        g.objs.aliens[idx as usize].roty = yaw;
        if yaw == target && notdelay(g, 4) {
            let pitch = strat_pitch_toward(&me, &p);
            // weapon_rot #0,#0 uses object aim (already negated).
            strat_fire_relslowlaserhome(g, idx, pitch, yaw);
        }
    }
    bossbrobouch_srou(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrobfire2_init` / `bossBrobfire2_strat` (GB3STRAT.ASM:2398).
pub fn bossbrobfire2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfire2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    bossbrobfire2_strat(g, idx);
}

/// Restore saved yaw, then hand off to jump1 (GB3STRAT.ASM:2403).
pub fn bossbrobfire2_strat(g: &mut Game, idx: u16) {
    let target = g.objs.aliens[idx as usize].sbyte2;
    let mut yaw = g.objs.aliens[idx as usize].roty;
    let _ = achase_angle(&mut yaw, target, 2);
    g.objs.aliens[idx as usize].roty = yaw;
    if yaw == target {
        bossbrobjump1_init(g, idx);
        return;
    }
    bossbrobouch_srou(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrobfrontplayer_srou` (GB3STRAT.ASM:2440): recenter XY then Z.
pub fn bossbrobfrontplayer_srou(g: &mut Game, idx: u16, front: i16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, 0, 3);
        al.worldy = achase16(al.worldy, -80 << BOSSB_SCALE, 3);
    }
    bossbrobfrontplayerz_srou(g, idx, front);
}

/// ROM `bossBrobfrontplayerZ_srou` (GB3STRAT.ASM:2444): aim yaw + chase Z.
pub fn bossbrobfrontplayerz_srou(g: &mut Game, idx: u16, front: i16) {
    if let Some(p) = player(g) {
        let m = g.objs.aliens[idx as usize];
        // ROM `s_obj2obj_angle` — Yanglexy+nega into body roty.
        let yaw = angle_xz(&m, &p).wrapping_neg();
        achase_angle(&mut g.objs.aliens[idx as usize].roty, yaw, 3);
        let target_z = p.worldz.wrapping_add(front);
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = achase16(al.worldz, target_z, 3);
    }
}

/// Shared ouch + bossHP + playerZ tail (`bossBrob_cont` / `bossBaddpz_cont`).
pub fn bossbrob_cont(g: &mut Game, idx: u16) {
    bossbrobouch_srou(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

fn bossbrob_ground_y() -> i16 {
    (-80i16) << BOSSB_SCALE
}

/// ROM `bossBrobstart_init` / `bossBrobstart_strat` (GB3STRAT.ASM:2288).
pub fn bossbrobstart_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobstart_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.sflags3 &= !ASF3_SFLAG5; // ROM clr sflag5 (ouch latch)
        al.sflags &= !ASF_NOHITAFFECT;
    }
    bossbrobstart_strat(g, idx);
}

/// Fall onto ground (`s_falldown_Yvec`), then enter the attack rotation.
pub fn bossbrobstart_strat(g: &mut Game, idx: u16) {
    let ground = bossbrob_ground_y();
    if fall_down_y_velocity(
        &mut g.objs.aliens[idx as usize],
        BOSSBROB_FALL_BOUNCE_SHIFT,
        BOSSBROB_FALL_GRAVITY,
        ground,
    ) {
        bossbrob_nextstate(g, idx);
        return;
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrobstart2_init` (GB3STRAT.ASM:2298) — hand off into fire1 spray.
pub fn bossbrobstart2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfire1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte2 = (0i8.wrapping_sub(DEG45 as i8)) as u8; // #-deg45
        al.sbyte1 = 60;
        al.sword1 = 0;
    }
    bossbrobfire1_strat(g, idx);
}

/// ROM `bossBrobjump1_init` (GB3STRAT.ASM:2308) — crouch anim, then jump2.
pub fn bossbrobjump1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobjump1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        init_animation(al, BOSSBROB_CROUCH_FRAME);
    }
    bossbrobjump1_strat(g, idx);
}

pub fn bossbrobjump1_strat(g: &mut Game, idx: u16) {
    if add_animation_cap(
        &mut g.objs.aliens[idx as usize],
        1,
        BOSSBROB_ANIMATION_FRAMES,
    ) {
        bossbrobjump2_init(g, idx);
        return;
    }
    bossbrob_cont(g, idx);
}

/// ROM `bossBrobjump2_init` (GB3STRAT.ASM:2321) — launch upward.
pub fn bossbrobjump2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobjump2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vel = 100;
    }
    gen_vecs_yaw(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].vy = -100; // after gen_vecs (ASM order)
    g.hooks.play_se(BOSSBROB_JUMP_SOUND);
    bossbrobjump2_strat(g, idx);
}

pub fn bossbrobjump2_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let ground = bossbrob_ground_y();
    if g.objs.aliens[idx as usize].worldy >= ground {
        bossbrobland_init(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(20);
        al.roty = al.roty.wrapping_add(7);
    }
    bossbrob_cont(g, idx);
}

/// ROM `bossBrobland_init` (GB3STRAT.ASM:2343) — land pause then fire1.
pub fn bossbrobland_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobland_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 10;
        al.sword1 = al.sword1.wrapping_add(1);
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
    }
    g.hooks.play_se(BOSSBROB_LAND_SOUND);
    bossbrobland_strat(g, idx);
}

pub fn bossbrobland_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrobfire1_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    if animation_frame(&g.objs.aliens[idx as usize]) != BOSSBROB_CROUCH_FRAME {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            BOSSBROB_ANIMATION_FRAMES,
        );
    } else if g.objs.aliens[idx as usize].sword1 == 2 {
        bossbrob_nextstate(g, idx);
        return;
    }
    bossbrob_cont(g, idx);
}

/// ROM `bossBrobfarjump1_init` (GB3STRAT.ASM:2732) — crouch + nohitaffect.
pub fn bossbrobfarjump1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfarjump1_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.shape = SH_BOSS_B_0;
        init_animation(al, BOSSBROB_CROUCH_FRAME);
        al.sflags |= ASF_NOHITAFFECT;
    }
    bossbrobfarjump1_strat(g, idx);
}

pub fn bossbrobfarjump1_strat(g: &mut Game, idx: u16) {
    if add_animation_cap(
        &mut g.objs.aliens[idx as usize],
        1,
        BOSSBROB_ANIMATION_FRAMES,
    ) {
        bossbrobfarjump2_init(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.roty, 0, 2);
    }
    bossbrob_cont(g, idx);
}

/// ROM `bossBrobfarjump2_init` (GB3STRAT.ASM:2748) — leap toward player.
pub fn bossbrobfarjump2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfarjump2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vel = 100;
    }
    gen_vecs_yaw(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].vy = -100; // after gen_vecs (ASM order)
    bossbrobfarjump2_strat(g, idx);
}

pub fn bossbrobfarjump2_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let ground = bossbrob_ground_y();
    if g.objs.aliens[idx as usize].worldy >= ground {
        bossbrobfarland_init(g, idx);
        return;
    }
    let px = player(g).map(|p| p.worldx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(8);
        achase_angle(&mut al.roty, DEG180, 3);
        if let Some(px) = px {
            al.worldx = achase16(al.worldx, px, 2);
        }
    }
    bossbrob_cont(g, idx);
}

/// ROM `bossBrobfarland_init` / `strat` (GB3STRAT.ASM:2770) — wait until close.
pub fn bossbrobfarland_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfarland_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        al.sflags |= ASF_NOHITAFFECT;
    }
    bossbrobfarland_strat(g, idx);
}

pub fn bossbrobfarland_strat(g: &mut Game, idx: u16) {
    if animation_frame(&g.objs.aliens[idx as usize]) != BOSSBROB_CROUCH_FRAME {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            BOSSBROB_ANIMATION_FRAMES,
        );
    }
    if zdist_less(g, idx, 2000) {
        g.objs.aliens[idx as usize].sflags &= !ASF_NOHITAFFECT;
        bossbrob_nextstate(g, idx);
        return;
    }
    bossbrobouch_srou(g, idx);
    add_bosshp(g, idx);
}

/// ROM `bossBrobkick_init` / `strat` (GB3STRAT.ASM:2696).
pub fn bossbrobkick_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobkick_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.shape = SH_BOSS_B_6;
    al.sbyte1 = 20;
    init_animation(al, BOSSBROB_IDLE_FRAME);
    bossbrobkick_strat(g, idx);
}

pub fn bossbrobkick_strat(g: &mut Game, idx: u16) {
    if animation_frame(&g.objs.aliens[idx as usize]) != BOSSBROB_FINAL_FRAME {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            1,
            BOSSBROB_ANIMATION_FRAMES,
        );
    }
    if animation_frame(&g.objs.aliens[idx as usize]) == BOSSBROB_KICK_FRAME {
        if let Some(foot) = make_obj(g, SH_BOSS_B_7) {
            copy_pos(g, foot, idx);
            copy_rots(g, foot, idx);
            {
                let al = &mut g.objs.aliens[foot as usize];
                al.worldx = al.worldx.wrapping_add(71i16 << BOSSB_SCALE);
                al.worldz = al.worldz.wrapping_sub(90i16 << BOSSB_SCALE);
            }
            bossbrobfoot_istrat(g, foot);
            g.hooks.play_se(BOSSBROB_MOVE_SOUND);
        }
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.rotz, 0, 2);
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrob_nextstate(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    bossbrob_cont(g, idx);
}

/// ROM `bossBrobMent_srou` (GB3STRAT.ASM:2059) — spawn linked split part.
pub fn bossbrobment_srou(g: &mut Game, mother: u16, child_num: u8) -> Option<u16> {
    bossbrob_ment(g, mother, child_num)
}

/// ROM `bossBrobMent2_srou` (GB3STRAT.ASM:2146) — ment + bossBent trail strat.
pub fn bossbrobment2_srou(g: &mut Game, mother: u16) -> Option<u16> {
    let anim = g.objs.aliens[mother as usize].animframe;
    let child = bossbrob_ment(g, mother, 0)?;
    let init = sid(g, bossbent_istrat);
    {
        let al = &mut g.objs.aliens[child as usize];
        al.stratptr = Some(init);
        al.type_ |= ATGND;
        init_animation(al, anim);
    }
    Some(child)
}

/// ROM `bossBent_Istrat` (GB3STRAT.ASM:1787) — fading trail.
pub fn bossbent_istrat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].count = 8;
    bossbent_icont(g, idx);
}

/// ROM `bossBentlong_Istrat` (GB3STRAT.ASM:1774) — longer trail life.
pub fn bossbentlong_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbent_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 20;
        al.sflags |= ASF_COLLDISABLE;
        al.stratptr = Some(tick);
        al.depthoffset = 1;
    }
    bossbent_strat(g, idx);
}

/// ROM `bossBent_Icont`.
pub fn bossbent_icont(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbent_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = Some(tick);
    al.depthoffset = 1;
    bossbent_strat(g, idx);
}

/// ROM `bossBent_strat` (GB3STRAT.ASM:1795).
pub fn bossbent_strat(g: &mut Game, idx: u16) {
    if notdelay_stag(g, idx, 3) {
        g.objs.aliens[idx as usize].depthoffset =
            g.objs.aliens[idx as usize].depthoffset.wrapping_add(1);
    }
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = c;
    if c == 0 {
        g.objs.aldead = 1;
        return;
    }
    bossbent_cont(g, idx);
}

/// ROM `bossBent_cont` (GB3STRAT.ASM:1802).
pub fn bossbent_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    boss_keeprel_to_player(g, idx);
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(1);
}

/// ROM `bossBrobfoot_Istrat` / `strat` (GB3STRAT.ASM:2797) — kicked foot projectile.
pub fn bossbrobfoot_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobfoot_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = None;
        al.expstratptr = None;
        al.hp = HARDHP;
        al.ap = 8;
        al.vel = 120;
        al.depthoffset = 1;
    }
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 0);
    }
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    bossbrobfoot_strat(g, idx);
}

pub fn bossbrobfoot_strat(g: &mut Game, idx: u16) {
    if notdelay(g, 1) {
        if let Some(trail) = bossbrobment2_srou(g, idx) {
            let init = sid(g, bossbentlong_istrat);
            g.objs.aliens[trail as usize].stratptr = Some(init);
        }
    }
    add_player_z(g, idx);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
}

/// ROM `bossBrobPouncepos_init` / `strat` (GB3STRAT.ASM:2459).
pub fn bossbrobpouncepos_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobpouncepos_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 30;
        init_animation(al, BOSSBROB_CROUCH_FRAME);
    }
    bossbrobpouncepos_strat(g, idx);
}

pub fn bossbrobpouncepos_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        g.objs.aliens[idx as usize].sbyte1 = 1;
        if add_animation_cap(
            &mut g.objs.aliens[idx as usize],
            1,
            BOSSBROB_ANIMATION_FRAMES,
        ) {
            bossbrobpounce2_init(g, idx);
            return;
        }
        if notdelay(g, 1) {
            let _ = bossbrobment2_srou(g, idx);
        }
    }
    bossbrobfrontplayer_srou(g, idx, 3000);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrobPounce2_init` / `strat` (GB3STRAT.ASM:2482).
pub fn bossbrobpounce2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobpounce2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vx = 0;
        al.vy = -75;
        al.vz = 0;
        al.rotx = 8;
    }
    g.hooks.play_se(BOSSBROB_JUMP_SOUND);
    bossbrobpounce2_strat(g, idx);
}

pub fn bossbrobpounce2_strat(g: &mut Game, idx: u16) {
    // ASM: if NOT in front of player, and |dz|>=500 → reappear.
    if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let in_front = me.worldz >= p.worldz; // s_jmp_objinfront x,y
        let far = (me.worldz as i32 - p.worldz as i32).abs() >= 500;
        if !in_front && far {
            bossbrobreappear_init(g, idx);
            return;
        }
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.rotx != 0 {
            al.rotx = al.rotx.wrapping_add(8);
        }
    }
    let ground = bossbrob_ground_y();
    if g.objs.aliens[idx as usize].worldy >= ground {
        let landed_frame = animation_frame(&g.objs.aliens[idx as usize]);
        if landed_frame == BOSSBROB_FINAL_FRAME {
            g.hooks.play_se(BOSSBROB_LAND_SOUND);
        }
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        if animation_frame(al) != BOSSBROB_CROUCH_FRAME {
            add_animation_wrap(al, -1, BOSSBROB_ANIMATION_FRAMES);
        }
        add_bosshp(g, idx);
        add_player_z(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(4);
    if notdelay(g, 1) {
        let _ = bossbrobment2_srou(g, idx);
    }
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// ROM `bossBrobreappear_init` / `strat` (GB3STRAT.ASM:2525).
pub fn bossbrobreappear_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobreappear_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.vx = 0;
        al.vy = -75;
        al.vz = 45;
        al.rotx = 8;
        init_animation(al, BOSSBROB_FINAL_FRAME);
    }
    bossbrobreappear_strat(g, idx);
}

pub fn bossbrobreappear_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.rotx != 0 {
            al.rotx = al.rotx.wrapping_add(8);
        }
    }
    let ground = bossbrob_ground_y();
    if g.objs.aliens[idx as usize].worldy >= ground {
        let landed_frame = animation_frame(&g.objs.aliens[idx as usize]);
        if landed_frame == BOSSBROB_FINAL_FRAME {
            g.hooks.play_se(BOSSBROB_LAND_SOUND);
        }
        let al = &mut g.objs.aliens[idx as usize];
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        if animation_frame(al) != BOSSBROB_CROUCH_FRAME {
            add_animation_wrap(al, -1, BOSSBROB_ANIMATION_FRAMES);
            bossbrob_cont(g, idx);
            return;
        }
        bossbrob_nextstate(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].vy = g.objs.aliens[idx as usize].vy.wrapping_add(3);
    if notdelay(g, 1) {
        let _ = bossbrobment2_srou(g, idx);
    }
    bossbrob_cont(g, idx);
}

/// Random-position table (GB3STRAT.ASM:2683) — (dx, dz) relative to player.
const BOSSBROB_RNDPOS_TAB: [(i16, i16); 8] = [
    (-100, 1200),
    (100, 1200),
    (-200, 2200),
    (200, 2200),
    (-300, 2700),
    (300, 2700),
    (-400, 3700),
    (400, 3700),
];

/// ROM `bossBrobrndpos_Istrat` (GB3STRAT.ASM:2623).
pub fn bossbrobrndpos_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobrndpos_strat);
    bossbrobrndpos_icont(g, idx, tick);
}

/// ROM `bossBrobrndpos2_Istrat` (GB3STRAT.ASM:2627) — body after sep.
pub fn bossbrobrndpos2_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobrndpos2_strat);
    bossbrobrndpos_icont(g, idx, tick);
}

fn bossbrobrndpos_icont(g: &mut Game, idx: u16, tick: StratId) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 1;
        al.sflags &= !ASF_NOHITAFFECT;
        al.sflags2 &= !ASF2_SFLAG1;
    }
    if let Some(s) = g.objs.aliens[idx as usize].stratptr {
        g.call_strat(s, idx);
    }
}

/// ROM `bossBrobrndpos2_strat` (GB3STRAT.ASM:2636) — ouch + HP then cont.
pub fn bossbrobrndpos2_strat(g: &mut Game, idx: u16) {
    bossbrobouch_srou(g, idx);
    add_bosshp(g, idx);
    bossbrobrndpos_cont(g, idx);
}

/// ROM `bossBrobrndpos_strat` (GB3STRAT.ASM:2644).
pub fn bossbrobrndpos_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.rotx, 0, 2);
        achase_angle(&mut al.rotz, 0, 2);
    }
    bossbrobrndpos_cont(g, idx);
}

/// Shared chase/fire tail (`bossBrobrndpos_cont`).
fn bossbrobrndpos_cont(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte1 = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        g.hooks.play_se(BOSSBROB_MOVE_SOUND);
        g.objs.aliens[idx as usize].sbyte1 = 30;
        let pick = (sf_random(&mut g.vars) as usize) & 7;
        let (dx, dz) = BOSSBROB_RNDPOS_TAB[pick];
        let px = player(g).map(|p| p.worldx).unwrap_or(0);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sword1 = px.wrapping_add(dx);
            al.sword2 = dz; // relative Z offset; cont adds player_posz
        }
    }
    let target_x = g.objs.aliens[idx as usize].sword1;
    let dz_off = g.objs.aliens[idx as usize].sword2;
    let target_z = player(g)
        .map(|p| p.worldz.wrapping_add(dz_off))
        .unwrap_or(dz_off);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, target_x, 2);
        al.worldz = achase16(al.worldz, target_z, 2);
        al.worldy = achase16(al.worldy, bossbrob_ground_y(), 3);
    }
    if notdelay_stag(g, idx, 4) {
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            // ROM RELSLOWELASERHOME → lasersound_l.
            strat_fire_relslowlaserhome(g, idx, pitch, yaw);
        }
    }
    if let Some(p) = player(g) {
        let m = g.objs.aliens[idx as usize];
        // ROM `s_obj2obj_angle` — Yanglexy+nega into body roty.
        let yaw = angle_xz(&m, &p).wrapping_neg();
        achase_angle(&mut g.objs.aliens[idx as usize].roty, yaw, 1);
    }
    add_player_z(g, idx);
}

/// ROM `bossBrobsep_init` (GB3STRAT.ASM:2587): spin, spawn rndpos parts, then rndpos2.
pub fn bossbrobsep_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobsep_strat);
    let coll = sid(g, bossbrobsepcol_istrat);
    let exp = sid(g, bossbrobsepexp_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.sbyte1 = 2;
        al.sbyte2 = 2;
        al.sflags2 &= !ASF2_SFLAG1;
        al.sflags |= ASF_NOHITAFFECT;
    }
    g.hooks.play_se(BOSSBROB_MOVE_SOUND);
    bossbrobsep_strat(g, idx);
}

/// ROM `bossBrobsep_strat` — yaw spin + ment→rndpos, then rndpos2.
pub fn bossbrobsep_strat(g: &mut Game, idx: u16) {
    if notdelay(g, 1) && animation_frame(&g.objs.aliens[idx as usize]) != BOSSBROB_CROUCH_FRAME {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            1,
            BOSSBROB_ANIMATION_FRAMES,
        );
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.roty != DEG180 {
            al.roty = al.roty.wrapping_add(4);
        }
    }
    // When close enough, spawn ment→rndpos children then finish to rndpos2.
    if !zdist_more(g, idx, 2000) {
        if g.objs.aliens[idx as usize].sbyte1 == 0 {
            bossbrobrndpos2_istrat(g, idx);
            return;
        }
        g.objs.aliens[idx as usize].sbyte1 -= 1;
        if let Some(child) = bossbrobment2_srou(g, idx) {
            g.objs.aliens[child as usize].sflags |= ASF_COLLDISABLE;
            let init = sid(g, bossbrobrndpos_istrat);
            g.objs.aliens[child as usize].stratptr = Some(init);
        }
    }
    bossbrobvecs_cont(g, idx);
}

// ---- Ouch damage reaction ---------------------------------------------------

/// bossBOuch_tab (GB3STRAT.ASM:2977-3025): 3 zones (left/right/top) × 16 frames
/// of (rotx,roty) knockback deltas. Flattened here; index = sbyte4 (a byte
/// offset into the table, +2 per tick as a (rotx,roty) pair).
const BOSSB_OUCH_TAB: [(i8, i8); 48] = [
    // left (offset 0)
    (6, 16),
    (5, 12),
    (4, 8),
    (3, 4),
    (-4, -10),
    (-4, -10),
    (-4, -8),
    (-3, -6),
    (-2, -4),
    (-1, -2),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    // right (offset 32)
    (6, -16),
    (5, -12),
    (4, -8),
    (3, -4),
    (-4, 10),
    (-4, 10),
    (-4, 8),
    (-3, 6),
    (-2, 4),
    (-1, 2),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    // top (offset 64)
    (-10, 0),
    (-10, 0),
    (-10, 0),
    (-8, 0),
    (-6, 0),
    (-2, 0),
    (2, 0),
    (6, 0),
    (8, 0),
    (10, 0),
    (10, 0),
    (10, 0),
    (0, 0),
    (0, 0),
    (0, 0),
    (0, 0),
];

/// ROM `bossBrobOuch_srou` (GB3STRAT.ASM:3055): while sbyte3>0 (in an Ouch),
/// apply the current (rotx,roty) knockback frame, step the frame, and lob a
/// BOSSHMISSILE1 counter-shot on a gate. sbyte4 = zone byte-offset (0/32/64) +
/// 2 per frame. When the count expires, ease rotx back to 0.
pub fn bossbrobouch_srou(g: &mut Game, idx: u16) {
    // ROM sflag5 ouching is ASF3_SFLAG5; gate on sbyte3>0.
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        return;
    }
    let entry = (g.objs.aliens[idx as usize].sbyte4 as usize) & 63;
    let (drx, dry) = BOSSB_OUCH_TAB[entry.min(47)];
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte4 = al.sbyte4.wrapping_add(2);
        if al.sbyte3 != 0 {
            al.sbyte3 -= 1;
        }
    }
    if g.objs.aliens[idx as usize].sbyte3 == 0 {
        // bossBrobOuchend: ease rotx back, clear the ouch flag.
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.rotx, 0, 3);
        al.sflags3 &= !ASF3_SFLAG5;
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(drx as u8);
        al.roty = al.roty.wrapping_add(dry as u8);
    }
    if notdelay(g, 4) {
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            fire_boss_hmissile1_aimed(g, idx, pitch, yaw);
        }
    }
}

// ---- death ------------------------------------------------------------------

/// sflags3 latch: walking-form morph already entered (chg4 / start).
const BOSSBROB_WALKING: u8 = 0x01;
const BOSSBROB_WALK_HP: u8 = 60; // bossBrobchg4 caps al_hp at 60 (GB3STRAT.ASM:2275)

/// Compatibility alias: first death → morph chain; second → sepexp.
pub fn bossbrob_transform_init(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags3 & BOSSBROB_WALKING == 0 {
        bossbrobchg_istrat(g, idx);
        return;
    }
    bossbrobsepexp_istrat(g, idx);
}

// ---- morph chain (GB3STRAT.ASM:2178-2285) ------------------------------------

/// ROM `bossBrobvecs_cont` — playerZ + gen_3dvecs + addvecs + bossHP.
pub fn bossbrobvecs_cont(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    bossbrobvecs_cont2(g, idx);
}

/// ROM `bossBrobvecs_cont2` — gen_3dvecs + addvecs + bossHP.
pub fn bossbrobvecs_cont2(g: &mut Game, idx: u16) {
    move_3d(&mut g.objs.aliens[idx as usize]);
    bossbrobvecs_cont4(g, idx);
}

/// ROM `bossBrobvecs_cont3` — addvecs + bossHP (vel already set).
pub fn bossbrobvecs_cont3(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    bossbrobvecs_cont4(g, idx);
}

/// ROM `bossBrobvecs_cont4` — boss HP and the damaged-form smoke trail.
pub fn bossbrobvecs_cont4(g: &mut Game, idx: u16) {
    add_bosshp(g, idx);
    if g.objs.aliens[idx as usize].hp <= BOSSBROB_DAMAGE_SMOKE_HP {
        g.objs.aliens[idx as usize].flags |= AFONFIRE;
        if notdelay(g, BOSSBROB_DAMAGE_SMOKE_PERIOD_BITS) {
            let _ = makesmoke_srou(g, idx);
        }
    }
}

/// ROM `bossBrobchg_Istrat` / `bossBrobchg_strat` (GB3STRAT.ASM:2178).
pub fn bossbrobchg_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobchg_strat);
    let coll = sid(g, bossbrobcol_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = 2;
        al.ap = 16;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = None;
        al.sbyte1 = 40;
        al.sflags2 |= ASF2_SFLAG3; // remove images
        al.sflags |= ASF_NOHITAFFECT;
        al.sflags3 |= BOSSBROB_WALKING; // latch so a second death → sepexp
    }
    g.hooks.play_music(BOSSBROB_TRANSFORM_MUSIC);
    g.hooks.play_se(BOSSB_TRANSFORM_SOUND);
    bossbrobchg_strat(g, idx);
}

pub fn bossbrobchg_strat(g: &mut Game, idx: u16) {
    achase_angle(&mut g.objs.aliens[idx as usize].roty, DEG180, 3);
    bossbrobcent_srou(g, idx);
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrobchg2_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    add_player_z(g, idx);
    bossbrobvecs_cont4(g, idx);
}

/// ROM `bossBrobchg2_init` / `strat` (GB3STRAT.ASM:2204).
pub fn bossbrobchg2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobchg2_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 0;
        al.stratptr = Some(tick);
        al.rotx = 0;
        al.roty = DEG180.wrapping_sub(DEG11);
    }
    g.hooks.play_se(BOSSBROB_APPROACH_SOUND);
    bossbrobchg2_strat(g, idx);
}

pub fn bossbrobchg2_strat(g: &mut Game, idx: u16) {
    let _ = speed_to(&mut g.objs.aliens[idx as usize], 60, 1);
    if zdist_less(g, idx, 1000) {
        bossbrobchg3_init(g, idx);
        return;
    }
    bossbrobvecs_cont(g, idx);
}

/// ROM `bossBrobchg3_init` / `strat` (GB3STRAT.ASM:2229).
pub fn bossbrobchg3_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobchg3_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    bossbrobchg3_strat(g, idx);
}

pub fn bossbrobchg3_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotz = al.rotz.wrapping_add(4);
        if al.roty != DEG45 {
            al.roty = al.roty.wrapping_add(4);
        }
    }
    if zdist_more(g, idx, 1400) {
        bossbrobchg4_init(g, idx);
        return;
    }
    bossbrobvecs_cont(g, idx);
}

/// ROM `bossBrobchg4_init` / `strat` (GB3STRAT.ASM:2251) — legs form.
pub fn bossbrobchg4_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobchg4_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.shape = SH_BOSS_B_0;
        init_animation(al, BOSSBROB_IDLE_FRAME);
        al.sbyte1 = 30;
    }
    g.hooks.play_music(6);
    bossbrobchg4_strat(g, idx);
}

pub fn bossbrobchg4_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.rotx, 0, 3);
        achase_angle(&mut al.rotz, 0, 3);
        al.worldx = achase16(al.worldx, 0, 4);
    }
    if !speed_to(&mut g.objs.aliens[idx as usize], 0, 2) {
        bossbrobvecs_cont(g, idx);
        return;
    }
    // .ycent — chase yaw to deg180, then anim / HP ramp.
    let yaw_done = {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.roty, DEG180, 2);
        al.roty == DEG180
    };
    if !yaw_done {
        bossbrobvecs_cont(g, idx);
        return;
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrobstart_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    {
        let hp = {
            let al = &mut g.objs.aliens[idx as usize];
            al.hp = al.hp.saturating_add(2); // up to 60
            al.hp
        };
        set_bossmaxhp(g, hp as u16);
    }
    if notdelay(g, 1) {
        if add_animation_cap(
            &mut g.objs.aliens[idx as usize],
            1,
            BOSSBROB_FORM_ANIMATION_FRAMES,
        ) {
            bossbrobvecs_cont(g, idx);
            return;
        }
        let _ = bossbrobment2_srou(g, idx);
        bossbrobvecs_cont2(g, idx);
        return;
    }
    bossbrobvecs_cont(g, idx);
}

/// ROM `bossBrobdemo_Istrat` / `strat` (GB3STRAT.ASM:1846) — intro pose machine.
pub fn bossbrobdemo_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobdemo_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = DEG180;
        init_animation(al, BOSSBROB_IDLE_FRAME);
        al.stratptr = Some(tick);
        al.shape = SH_BOSS_B_0;
        al.sbyte1 = 35;
        al.sflags |= ASF_SHADOW;
        al.worldy = bossbrob_ground_y();
        al.stratstate = 0;
    }
    bossbrobdemo_strat(g, idx);
}

pub fn bossbrobdemo_strat(g: &mut Game, idx: u16) {
    let state = g.objs.aliens[idx as usize].stratstate;
    match state {
        0 => {
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 1;
            } else {
                g.objs.aliens[idx as usize].sbyte1 -= 1;
            }
        }
        1 => {
            g.objs.aliens[idx as usize].sbyte1 = 20;
            if notdelay(g, 1) {
                if animation_frame(&g.objs.aliens[idx as usize]) == BOSSBROB_CROUCH_FRAME {
                    g.objs.aliens[idx as usize].stratstate = 2;
                } else {
                    add_animation_wrap(
                        &mut g.objs.aliens[idx as usize],
                        1,
                        BOSSBROB_FORM_ANIMATION_FRAMES,
                    );
                    let _ = bossbrobment2_srou(g, idx);
                }
            }
        }
        2 => {
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 3;
            } else {
                g.objs.aliens[idx as usize].sbyte1 -= 1;
            }
        }
        3 => {
            g.objs.aliens[idx as usize].sbyte1 = 20;
            let _ = bossbrobment2_srou(g, idx);
            if animation_frame(&g.objs.aliens[idx as usize]) == BOSSBROB_FINAL_FRAME {
                g.objs.aliens[idx as usize].stratstate = 4;
            } else {
                add_animation_wrap(
                    &mut g.objs.aliens[idx as usize],
                    1,
                    BOSSBROB_ANIMATION_FRAMES,
                );
            }
        }
        4 => {
            let _ = bossbrobment2_srou(g, idx);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.rotx = al.rotx.wrapping_sub(DEG180 / 10); // deg360/20
            }
            if g.objs.aliens[idx as usize].sbyte1 == 0 {
                g.objs.aliens[idx as usize].stratstate = 5;
            } else {
                g.objs.aliens[idx as usize].sbyte1 -= 1;
            }
        }
        5 => {
            let _ = bossbrobment2_srou(g, idx);
            {
                let al = &mut g.objs.aliens[idx as usize];
                al.sbyte1 = 50;
                al.rotx = 0;
                if animation_frame(al) == BOSSBROB_CROUCH_FRAME {
                    al.stratstate = 6;
                } else {
                    add_animation_wrap(al, -1, BOSSBROB_ANIMATION_FRAMES);
                }
            }
        }
        6 => {
            {
                let al = &mut g.objs.aliens[idx as usize];
                if al.sbyte1 != 0 {
                    al.sbyte1 -= 1;
                }
                if al.sbyte1 == 0 {
                    al.sbyte1 = 1;
                }
            }
            if g.objs.aliens[idx as usize].sbyte1 == 1 && notdelay(g, 1) {
                aim_and_fire_laser(g, idx, 0);
            }
        }
        _ => {}
    }
}

/// ROM `bossBrobundead_Istrat` / `strat` (GB3STRAT.ASM:3089).
pub fn bossbrobundead_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobundead_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.hp = 32;
        al.vx = 0;
        al.vy = -40;
        al.vz = 40;
        al.sflags |= ASF_NOHITAFFECT;
    }
    bossbrobundead_strat(g, idx);
}

pub fn bossbrobundead_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        achase_angle(&mut al.roty, DEG180, 3);
        achase_angle(&mut al.rotx, (0i8.wrapping_sub(DEG90 as i8)) as u8, 3);
        let _ = fall_down_y_velocity(al, BOSSBROB_FALL_BOUNCE_SHIFT, BOSSBROB_FALL_GRAVITY, 0);
    }
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    add_player_z(g, idx);
}

/// ROM `bossBrobdie_init` / `strat` (GB3STRAT.ASM:3111).
pub fn bossbrobdie_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobdie_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.sbyte1 = 5;
        al.stratptr = Some(tick);
        al.type_ |= ATZREMOVE; // s_setremove_behind
    }
    bossbrobdie_strat(g, idx);
}

pub fn bossbrobdie_strat(g: &mut Game, idx: u16) {
    if let Some(e) = make_medium_exp_obj(g, idx) {
        addrnd2pos_xy(g, e);
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        strat_qboss_explode_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
}

// ============================================================
// bossB death explode chain (GB3STRAT.ASM:2850-2946)
// ============================================================

const SH_BOSS_B_L: u16 = 397;
const SH_BOSS_B_R: u16 = 398;
const SH_BOSS_B_H: u16 = 399;

/// ROM `bossBrobsepexp_Istrat` (GB3STRAT.ASM:2850) — fall, wait, then split.
pub fn bossbrobsepexp_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobsepexp_strat);
    let coll = sid(g, bossbrobcol_istrat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.sbyte1 = 40;
        al.sflags2 |= ASF2_SFLAG1;
        al.hp = 2;
        al.sflags |= ASF_COLLDISABLE;
    }
    g.hooks.play_se(BOSSB_TRANSFORM_SOUND);
    g.hooks.play_music(BOSSBROB_DEATH_MUSIC);
    bossbrobsepexp_strat(g, idx);
}

/// ROM `bossBrobsepexp_strat` — Lexp shower while falling; then bossBrobexp.
pub fn bossbrobsepexp_strat(g: &mut Game, idx: u16) {
    if let Some(e) = make_large_exp_obj(g, idx) {
        g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
        addrnd2pos_xy(g, e);
    }
    let ground = (-80i16) << BOSSB_SCALE;
    if g.objs.aliens[idx as usize].worldy < ground {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(8);
        apply_velocity(al);
    }
    // s_jmp_Zdistmore #1300,.nstop — only scroll when close.
    if let Some(p) = player(g) {
        let dz = (g.objs.aliens[idx as usize].worldz as i32 - p.worldz as i32).abs();
        if dz < BOSSBROB_DEATH_SCROLL_DISTANCE {
            add_player_z(g, idx);
        }
    }
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrobexp_init(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    {
        let mut rx = g.objs.aliens[idx as usize].rotx;
        let mut ry = g.objs.aliens[idx as usize].roty;
        achase_angle(&mut rx, 0, 2);
        achase_angle(&mut ry, DEG180, 2);
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = rx;
        al.roty = ry;
    }
}

/// ROM `bossBrobexp_init` (GB3STRAT.ASM:2882) — spawn L/R debris, self becomes head.
pub fn bossbrobexp_init(g: &mut Game, idx: u16) {
    // Left debris.
    if let Some(l) = make_obj(g, SH_BOSS_B_L) {
        copy_pos(g, l, idx);
        {
            let al = &mut g.objs.aliens[l as usize];
            al.roty = DEG180;
            al.worldx = al.worldx.wrapping_add(38i16 << BOSSB_SCALE);
            al.worldy = al.worldy.wrapping_add(45i16 << BOSSB_SCALE);
            al.count = 5;
            al.hp = HARDHP;
        }
        bossbpwaitexp_istrat(g, l);
        let s_exp = sid(g, bossbpexp_istrat);
        g.objs.aliens[l as usize].expstratptr = Some(s_exp);
    }
    // Right debris.
    if let Some(r) = make_obj(g, SH_BOSS_B_R) {
        copy_pos(g, r, idx);
        {
            let al = &mut g.objs.aliens[r as usize];
            al.roty = DEG180;
            al.worldx = al.worldx.wrapping_add(-(38i16 << BOSSB_SCALE));
            al.worldy = al.worldy.wrapping_add(45i16 << BOSSB_SCALE);
            al.count = 15;
            al.hp = HARDHP;
        }
        bossbpwaitexp_istrat(g, r);
        let s_exp = sid(g, bossbpexp_istrat);
        g.objs.aliens[r as usize].expstratptr = Some(s_exp);
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_BOSS_B_H;
        al.count = 25;
    }
    bossbpwaitexp_istrat(g, idx);
    let s_exp = sid(g, bossbpexp2_istrat);
    g.objs.aliens[idx as usize].expstratptr = Some(s_exp);
    // s_jmpto_strat — resume wait tick this frame.
    bossbpwaitexp_strat(g, idx);
}

/// ROM `bossBpwaitexp_Istrat` (GB3STRAT.ASM:2913).
pub fn bossbpwaitexp_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbpwaitexp_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.hp = HARDHP;
        al.ap = 10;
    }
}

/// ROM `bossBpwaitexp_strat` — occasional Lexp, then expire into expstrat.
pub fn bossbpwaitexp_strat(g: &mut Game, idx: u16) {
    if notdelay_stag(g, idx, 1) {
        if let Some(e) = make_large_exp_obj(g, idx) {
            g.objs.aliens[e as usize].sflags4 |= ASF4_NOPOLYEXP;
            addrnd2pos_xy(g, e);
        }
    }
    let c = g.objs.aliens[idx as usize].count.wrapping_sub(1);
    g.objs.aliens[idx as usize].count = c;
    add_player_z(g, idx);
    if c == 0 {
        // s_dec_lifecnt with remove — invoke expstrat if set.
        match g.objs.aliens[idx as usize].expstratptr {
            Some(exp) => g.call_strat(exp, idx),
            None => g.objs.aldead = 1,
        }
    }
}

/// ROM `bossBpexp2_Istrat` (GB3STRAT.ASM:2929) — bossdead + circle + FOL + tumble.
pub fn bossbpexp2_istrat(g: &mut Game, idx: u16) {
    g.vars.gameflags |= GF_BOSSDEAD;
    let _ = start_boss_explosion_circle(g, idx);
    let _ = make_fol_exp_obj(g, idx);
    let s = sid(g, bossbpexp_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    bossbpexp_strat(g, idx);
}

/// ROM `bossBpexp_Istrat` (GB3STRAT.ASM:2937) — Lexp then tumble.
pub fn bossbpexp_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, bossbpexp_strat);
    g.objs.aliens[idx as usize].expstratptr = Some(s);
    let _ = make_large_exp_obj(g, idx);
    bossbpexp_strat(g, idx);
}

/// ROM `bossBpexp_strat` — mark removable + spin (debris tumble).
pub fn bossbpexp_strat(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.type_ |= ATZREMOVE; // s_setremove_behind
    al.rotz = al.rotz.wrapping_add(8);
    al.rotx = al.rotx.wrapping_add(4);
}

// ============================================================
// Registration (table lane hookup).
// ============================================================

/// Populate the Andross `g_istrats` rows + the MAP1_5 synthetic address.
/// Called from `table::register_all` right after `enemies_ground::register`.
pub fn register(world: &mut World) {
    world.istrats[IS_BOSSB] = Some(wsid(world, bossb_init));
    world.istrats[IS_BOSSBROB] = Some(wsid(world, bossbrob_init));
    // MAP1_5.ASM places the face via the synthetic STRAT_ADDR_BOSSB (0x06000F);
    // the generic address-map loop only mints SYNTH_BASE|i / FLAT|i, so register
    // this explicit address here (mirrors enemy_b::STRAT_ADDR_BOSSF).
    if let Some(id) = world.istrats[IS_BOSSB] {
        world.register_strategy_address(STRAT_ADDR_BOSSB, id);
    }
    // bossBrob (IS 117) is spawned by the native MAP1_6A transcription.
}
