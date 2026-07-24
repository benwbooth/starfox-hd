//! bossH — the "gggy" walking/legged spider boss (RIIR port, ASM ground truth).
//!
//! Four strategy entities from `reference/ultrastarfox/SF/STRAT/D3STRATS.ASM`
//! (lines 34-931): `bossh_istrat` (the body/mother), `bosshleg_istrat` (five
//! child legs), `bosshtop_istrat` (the firing top), and `teleporter_istrat`
//! (a late-game cosmetic teleport prop). There is no C-oracle counterpart
//! (`strat_boss*.c` never ported bossH) and NO sf-oracle differential fixture,
//! so every cite below is to the retail assembly. The Andross module
//! (`bossb.rs`) is the structural template (child-linked multi-part boss in a
//! standalone module); private `bosses.rs` helpers are replicated locally per
//! the lane rules.
//!
//! ── ISTRAT / MAP WIRING ────────────────────────────────────────────────────
//! bossH has no `def_Istrat` row in ISTRATS.ASM (grep confirms: neither
//! `bossh`, `bosshleg`, `bosshtop`, nor `teleport` appears). It is placed by a
//! DIRECT strategy-address reference: `MAP1_4.ASM:217`
//!   `mapobj 0000,2000,-600,1000,boss_h_0,bossh_istrat`
//! The port represents that authored placement with the typed
//! `DirectStrategy::BossH` identity. The legs and top are spawned by the
//! mother and therefore need no map identity of their own.
//!
//! sf-map's MAP1_4 places the boss through `STRAT_ADDR_BOSSH`, so this strategy
//! is live on the route-1 Macbeth boss encounter.
//!
//! ── FIDELITY ───────────────────────────────────────────────────────────────
//! The native strategy retains the authored object graph and implements:
//!   • init + HP-bar model (s_set_var bosshhitcount = 5*2+5*5 = 35;
//!     s_set_bossmaxHP bosshhitcount + s_add_bossmaxHP #bosshHP = 99;
//!     each frame adds body HP and `bosshhitcount` to the displayed total);
//!   • the `.generate` mother + five child legs + top spawn (child slots
//!     1/3/5/2/4 = leg1..5, slot 6 = top) at their arrangement offsets;
//!   • the `bosshhitcount` PHASE GATE: −5 per scripted `droptoground`
//!     (D3:356) and −5 per leg destroyed (D3:853);
//!   • the `.move` tail's leg-dead → vulnerable transition: drop
//!     `nohitaffect` and use the red color table once every leg is gone;
//!     otherwise pin body HP so the mother remains invulnerable;
//!   • the complete leg animation/vulnerability machine, including the two
//!     source meshes, phase-specific collision response, fall, and explosion;
//!   • the top's roty-window fire gate (fires when facing ±deg22 forward on the
//!     notdelay-4 tick);
//!   • death → mother `.explode` (kill top + remove teleport → boss explosion).
//!   • the complete 22-entry mother choreography, leg-mode synchronization,
//!     teleport child, impact smoke, gait/rotation sounds, and canonical
//!     player-targeted HPLASMA construction.
//! `teleporter_istrat` + `fire_bonfire` cover D3:892-978.

#![allow(dead_code)]

use sf_game::alien::{
    Alien, ObjectVisualKind, StratId, ACF_COLLTYPE1, ACF_COLLTYPE2, ACF_COLLTYPE3, ACF_COLLTYPE4,
    ACF_COLLTYPE5, ACF_COLLTYPE6, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::{HARD_AP, HARD_HP};
use sf_game::world::World;

use crate::common::{
    add_colanim_wrap, init_colanim, sf_random, strat_apply_velocity as apply_velocity, strat_chase,
    strat_chase8, strat_chase_proportional, strat_gen_vecs_nvecs, strat_make_obj,
};
use crate::enemy_a::{
    add_player_z, boss_attach_child_to_mother, boss_find_child_obj, boss_get_mother_obj,
    defelasercol_istrat, fire_bonfire, fire_hplasma, player, strat_boss_explode_init,
    strat_explode, strat_hit_flash, ASF2_SFLAG1, ASF2_SFLAG2, ASF2_SFLAG3,
};

// ============================================================
// Constants — verbatim D3STRATS.ASM equs + VARS.INC / STRATEQU.INC.
// ============================================================
const BOSSH_HP: u8 = 64; // D3STRATS.ASM:34 bosshHP
const BOSSH_AP: u8 = 4; // D3STRATS.ASM:35 bosshAP
const BOSSHLEG_HP: u8 = 10; // D3STRATS.ASM:36 bosshlegHP
const BOSSHLEG_AP: u8 = 4; // D3STRATS.ASM:37 bosshlegAP
const HARDHP: u8 = 255; // STRATEQU.INC:68 hardHP == -1 (bosshtopHP)
const HARDAP: u8 = 8; // STRATEQU.INC:66 hardAP (bosshtopAP)
const BOSSHLEG_PROTECTED_HP: u8 = BOSSHLEG_HP + 64;
const BOSSHLEG_RAISE_HP_THRESHOLD: u8 = 64;

/// s_set_var bosshhitcount,#5*2+5*5 (D3STRATS.ASM:76): the phase gate seed —
/// 5 per scripted drop ×2 + 5 per leg ×5 = 35.
const BOSSHHITCOUNT_INIT: u8 = 5 * 2 + 5 * 5;

const DEG22: u8 = 16; // VARS.INC:15 deg22 = deg360/16
const DEG180: u8 = 128; // VARS.INC:12
const ASF2_SFLAG4: u8 = 0x80;

const BOSSH_WALK_Z_DISTANCE: i16 = 2500;
const BOSSH_WALK_X_STEP: i16 = -25;
const BOSSH_WALK_Z_STEP: i16 = 20;
const BOSSH_CLOSE_Z_DISTANCE: i16 = 500;
const BOSSH_NEAR_Z_DISTANCE: i16 = 1500;
const BOSSH_FAR_Z_DISTANCE: i16 = 2500;
const BOSSH_SCUTTLE_SWITCH_DISTANCE: i16 = 2000;
const BOSSH_BACKWARD_Z_STEP: i16 = 80;
const BOSSH_FORWARD_Z_STEP: i16 = -50;
const BOSSH_OSCILLATION_Z_STEP: i16 = 40;
const BOSSH_OSCILLATION_LOW_Y: i16 = -300;
const BOSSH_OSCILLATION_HIGH_Y: i16 = -70;
const BOSSH_FLIGHT_Y: i16 = -400;
const BOSSH_GROUND_Y: i16 = -80;
const BOSSH_FALL_GRAVITY: i16 = 8;
const BOSSH_FALL_BOUNCE_SHIFT: u32 = 4;
const BOSSH_SPIN_TARGET: u8 = 20;
const BOSSH_SPIN_CHASE_STEP: u8 = 1;
const BOSSH_SCUTTLE_SPIN_TARGET: u8 = 2;
const BOSSH_SCUTTLE_SPIN_CHASE_STEP: u8 = 3;
const BOSSH_SPIN_RISE_STEP: i16 = -4;
const BOSSH_HEIGHT_CHASE_STEP: i16 = 5;
const BOSSH_HEIGHT_CHASE_SHIFT: u32 = 3;
const BOSSH_FLIGHT_CHASE_SHIFT: u32 = 4;
const BOSSH_OSCILLATION_FAST_PHASE_STEP: u8 = 4;
const BOSSH_OSCILLATION_SLOW_PHASE_STEP: u8 = 2;
const BOSSH_OSCILLATION_SCALE: u32 = 2;
const BOSSH_STAND_TICKS: u8 = 15;
const BOSSH_LOOP_WAIT_TICKS: u8 = 30;
const BOSSH_IMPACT_GATE_DAMAGE: u8 = 5;
const BOSSH_GAIT_SOUND: u8 = 79;
const BOSSH_IMPACT_SOUND: u8 = 142;
const BOSSH_TELEPORT_SOUND: u8 = 152;
const BOSSH_LEG_SOUND: u8 = 151;
const BOSSH_LEG_HIT_SOUND: u8 = 36;
const BOSSH_HPLASMA_LIFETIME: u8 = 255;
const BOSSH_HPLASMA_MUZZLE_Y: i8 = -50;
const BOSSH_WEAPON_SCALE: u32 = 2;
const BOSSH_TOP_SPIN_STEP: u8 = 5;
const BOSSH_SMOKE_COUNT: usize = 3;
const BOSSH_SMOKE_SHAPE: u16 = 364;
const BOSSH_SMOKE_FRAMES: u8 = 8;
const BOSSH_SMOKE_FINAL_FRAME: u8 = 7;
const BOSSH_SMOKE_X_MASK: u8 = 31;
const BOSSH_SMOKE_X_CENTER: i8 = 15;
const BOSSH_SMOKE_X_SCALE: i16 = 2;
const BOSSH_RANDOM_PERCENT_SCALE: u16 = 255;
const PERCENT_DENOMINATOR: u16 = 100;
const BOSSH_RARE_SHAKE_PERCENT: u16 = 99;
const BOSSH_RANDOM_HALF_PERCENT: u16 = 50;
const BOSSH_MAX_IMMEDIATE_TRANSITIONS: usize = 24;
const BOSSH_LEG_MAX_IMMEDIATE_TRANSITIONS: usize = 4;
const BOSSH_LEG_FALL_SPEED: u8 = 120;
const BOSSH_LEG_FALL_GRAVITY: i16 = 4;
const BOSSH_LEG_FALL_GROUND_Y: i16 = -20;
const BOSSH_LEG_ALTERNATE_SHAPE_FRAME: u8 = 13;
const BOSSH_LEG_ANIMATION_FRAMES: u8 = 16;
const BOSSH_LEG_RAISE_LIMIT: u8 = 11;
const BOSSH_LEG_FLAT_LIMIT: u8 = 15;
const BOSSH_LEG_SCAMPER_HIGH_FRAME: u8 = 3;
const BOSSH_LEG_SCAMPER_LOW_FRAME: u8 = 0;
const BOSSH_LEG_SHAKE_HIGH_FRAME: u8 = 5;
const BOSSH_LEG_WAGGLE_HIGH_FRAME: u8 = 10;
const BOSSH_LEG_WAGGLE_LOW_FRAME: u8 = 9;
const BOSSH_LEG_SCAMPER_LOW_HIGH_FRAME: u8 = 11;
const BOSSH_LEG_SCAMPER_LOW_LOW_FRAME: u8 = 9;
const BOSSH_LEG_SCAMPER_MIDDLE_HIGH_FRAME: u8 = 6;
const BOSSH_LEG_SCAMPER_MIDDLE_LOW_FRAME: u8 = 4;
const BOSSH_LEG_MOVE_TO_THIRTY_FRAME: u8 = 11;
const BOSSH_LEG_LOWER_BASE_FRAME: u8 = 9;
const BOSSH_LEG_MIDDLE_BASE_FRAME: u8 = 4;
const BOSSH_LEG_CHILD_FRAME_MASK: u8 = 0x03;
const BOSSH_NOTDELAY_SPIN_BITS: u16 = 2;
const BOSSH_NOTDELAY_WEAPON_BITS: u16 = 4;
const BOSSH_NOTDELAY_LEG_BITS: u16 = 4;
const BOSSH_ROTATION_HALF_MASK: u8 = 0x80;
const BOSSH_RAISED_LEGS_TO_ADVANCE: usize = 3;
const BOSSH_TWO_RAISED_LEG_SPIN: u8 = 6;
const BOSSH_ONE_RAISED_LEG_SPIN: u8 = 4;
const BOSSH_VULNERABLE_TEXTURE_STEP: u8 = 10;
const BOSSH_TEXTURE_STEP: u8 = 5;
const BOSSH_Y_BOB_MASK: u16 = 0x03;
const BOSSH_Y_BOB: [i16; 4] = [-15, -5, 5, 15];
const TELEPORTER_START_DELAY: u8 = 50;
const TELEPORTER_FIRST_BONFIRE_TICK: u8 = 20;
const TELEPORTER_FINAL_BONFIRE_TICK: u8 = 1;
const TELEPORTER_RETRACT_FRAMES: u8 = 16;
const TELEPORTER_RISE_FRAMES: u8 = 10;
const TELEPORTER_COLLAPSE_FRAMES: u8 = 20;
const TELEPORTER_TEXTURE_RISE_STEP: u8 = 5;
const ANIMATION_ACTIVE: u8 = 0x80;
const ANIMATION_FRAME_MASK: u8 = 0x7f;
const COLLISION_TYPE_MASK: u8 =
    ACF_COLLTYPE1 | ACF_COLLTYPE2 | ACF_COLLTYPE3 | ACF_COLLTYPE4 | ACF_COLLTYPE5 | ACF_COLLTYPE6;

/// id_1_c coltab (red-hot palette the body/legs flip to when vulnerable). The
/// meshes aren't wired, so the exact value is cosmetic; a nonzero marker keeps
/// the ASM semantics observable.
const ID_1_C: u16 = 1;

// bossH_scale=2 (STRATEQU.INC:304), childscale=3 (STRATLIB.INC:668). Leg local
// offsets are `(N<<2)>>3` = `N>>1` (signed), gggy = (10<<2)>>3 = 5.
// ── D3STRATS.ASM:476-489 (.generate) ──────────────────────────────────────
// Child numbers (D3:59-65): leg1=1 leg2=3 leg3=5 leg4=2 leg5=4 top=6 teleport=7.
const BOSSH_LEG1: u8 = 1;
const BOSSH_LEG2: u8 = 3;
const BOSSH_LEG3: u8 = 5;
const BOSSH_LEG4: u8 = 2;
const BOSSH_LEG5: u8 = 4;
const BOSSH_TOP: u8 = 6;
const BOSSH_TELEPORT: u8 = 7;
const BOSSH_FIRST_LEG: u8 = 1;
const BOSSH_LAST_LEG: u8 = 5;
const BOSSH_LEG_COUNT: usize = 5;
const BOSSH_CHILD_POSITION_SCALE: u32 = 3;
// Direct-address component meshes have no ISTRATS def_shape rows.  Their
// stable extended ids are owned by tools/shape_compiler.py.
const SH_BOSSH_LEG: u16 = 301;
const SH_BOSSH_LEG_ALTERNATE: u16 = 302;
const SH_BOSSH_TOP: u16 = 303;
const SH_BOSSH_TELEPORT: u16 = 304;

/// Per-leg (child_num, offx, offy, offz, roty) from the five
/// s_make_childobjrotpos calls (D3:479-487). Angles: deg72=51 deg144=102
/// deg216=153 deg288=204 deg180=128 (deg360=256), all wrapped to u8.
/// leg1 roty=-deg180=128; leg2 -deg72-deg180=-179→77; leg3 -deg144-deg180=
/// -230→26; leg4 -deg216+deg180=-25→231; leg5 -deg288+deg180=-76→180.
const LEG_LAYOUT: [(u8, i16, i16, i16, u8); BOSSH_LEG_COUNT] = [
    (BOSSH_LEG1, 0, 5, 15, 128),
    (BOSSH_LEG2, 14, 5, 4, 77),
    (BOSSH_LEG3, 9, 5, -12, 26),
    (BOSSH_LEG4, -9, 5, -12, 231),
    (BOSSH_LEG5, -14, 5, 4, 180),
];

/// leg local offset by child_num (for the per-frame rotpos placement).
fn leg_offset(child_num: u8) -> Option<(i16, i16, i16)> {
    LEG_LAYOUT
        .iter()
        .find(|e| e.0 == child_num)
        .map(|e| (e.1, e.2, e.3))
}

// ── mother mode table (D3STRATS.ASM:85-117). Indices preserved so the phase
// gate + bh_looptohere land where the ROM puts them. ────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum BossHMode {
    WalkOn = 0,
    Spin = 1,
    FirstDrop = 2,
    FirstSpinUp = 3,
    FirstOscillation = 4,
    SetMiddlePose = 5,
    WaitForMiddlePose = 6,
    ScuttleForward = 7,
    ScuttleBackward = 8,
    WaitForRaisedLegs = 9,
    SecondDrop = 10,
    MakeLegsVulnerable = 11,
    SecondSpinUp = 12,
    AttackOscillation = 13,
    RetreatToFarDistance = 14,
    RiseToFlightHeight = 15,
    CreateTeleporter = 16,
    AdvanceToNearDistance = 17,
    WaitForTeleporter = 18,
    LoopDelay = 19,
    Stand = 20,
    Crouch = 21,
}

impl BossHMode {
    fn from_raw(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::WalkOn,
            1 => Self::Spin,
            2 => Self::FirstDrop,
            3 => Self::FirstSpinUp,
            4 => Self::FirstOscillation,
            5 => Self::SetMiddlePose,
            6 => Self::WaitForMiddlePose,
            7 => Self::ScuttleForward,
            8 => Self::ScuttleBackward,
            9 => Self::WaitForRaisedLegs,
            10 => Self::SecondDrop,
            11 => Self::MakeLegsVulnerable,
            12 => Self::SecondSpinUp,
            13 => Self::AttackOscillation,
            14 => Self::RetreatToFarDistance,
            15 => Self::RiseToFlightHeight,
            16 => Self::CreateTeleporter,
            17 => Self::AdvanceToNearDistance,
            18 => Self::WaitForTeleporter,
            19 => Self::LoopDelay,
            20 => Self::Stand,
            21 => Self::Crouch,
            _ => return None,
        })
    }

    fn next(self) -> Self {
        Self::from_raw((self as u8).wrapping_add(1)).unwrap_or(Self::AttackOscillation)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum BossHLegMode {
    Scampering = 0,
    BeginShake = 1,
    Shake = 2,
    Stand = 3,
    Crouch = 4,
    Raise = 5,
    Waggle = 6,
    MoveFlat = 7,
    MoveToThirty = 8,
    LowerPose = 9,
    Lowered = 10,
    ScamperLow = 11,
    MiddlePose = 12,
    Middled = 13,
    ScamperMiddle = 14,
}

impl BossHLegMode {
    fn from_raw(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Scampering,
            1 => Self::BeginShake,
            2 => Self::Shake,
            3 => Self::Stand,
            4 => Self::Crouch,
            5 => Self::Raise,
            6 => Self::Waggle,
            7 => Self::MoveFlat,
            8 => Self::MoveToThirty,
            9 => Self::LowerPose,
            10 => Self::Lowered,
            11 => Self::ScamperLow,
            12 => Self::MiddlePose,
            13 => Self::Middled,
            14 => Self::ScamperMiddle,
            _ => return None,
        })
    }

    fn next(self) -> Self {
        Self::from_raw((self as u8).wrapping_add(1)).unwrap_or(Self::Scampering)
    }
}

enum StrategyStep {
    Continue,
    Yield,
}

// ============================================================
// Registry identity.
// ============================================================

pub const STRATEGY_BOSSH: sf_map::consts::DirectStrategy = sf_map::consts::DirectStrategy::BossH;

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

// ============================================================
// STRATMAC / STRATLIB helpers (ASM-cited; local per lane rules).
// ============================================================

/// s_jmp_notdelay N (STRATMAC.INC): TRUE when `gameframe & ((1<<N)-1) == 0`.
fn notdelay(g: &Game, bits: u16) -> bool {
    g.vars.gameframe & ((1u16 << bits) - 1) == 0
}

/// Add the body's current HP to the per-frame boss-bar accumulator
/// (STRATLIB.INC:562). `init_strats` zeroes it before strategy dispatch.
fn add_bosshp_obj(g: &mut Game, idx: u16) {
    let hp = g.objs.aliens[idx as usize].hp as u16;
    g.vars.bosshp = g.vars.bosshp.wrapping_add(hp);
}
/// s_add_bossHP bosshhitcount (STRATLIB.INC:562, var form): m_bossHP += var.
fn add_bosshp_val(g: &mut Game, v: u8) {
    g.vars.bosshp = g.vars.bosshp.wrapping_add(v as u16);
}
/// s_set_bossmaxHP {var}/#v + s_add_bossmaxHP (STRATLIB.INC:519-643): set the
/// denominator and zero the accumulator.
fn set_bossmaxhp(g: &mut Game, v: u16) {
    g.vars.bossmaxhp = v;
    g.vars.bosshp = 0;
}

/// Retail child-death query (STRATLIB.INC:801): true when no child in the
/// inclusive authored range remains linked and alive.
fn children_dead(g: &mut Game, mother: u16, begin: u8, end: u8) -> bool {
    for n in begin..=end {
        if boss_find_child_obj(g, mother, n).is_some() {
            return false;
        }
    }
    true
}

/// Retail fall-and-bounce helper (STRATMAC.INC:1813). Twin of
/// enemies_ground::falldown_yvec: gravity onto al_vy, land at `ground`, bounce
/// = (-vy >> bounceyness) with the small-value clamp; returns landed (vy==0).
fn falldown_yvec(al: &mut Alien, bounceyness: u32, gravity: i16, ground: i16) -> bool {
    al.vy = al.vy.wrapping_add(gravity); // s_add_2Yvec
    if al.worldy < ground {
        return false; // s_jmp_higher — still airborne
    }
    al.worldy = ground; // s_set_alvar al_worldy,ground
    let mut v = al.vy.wrapping_neg() >> bounceyness;
    if (-5..=0).contains(&v) {
        v = 0;
    }
    al.vy = v;
    v == 0
}

/// The mother's bosshhitcount lives in the ROM as a global RAM byte. In this
/// port it is stored in the mother's spare `al_sbyte1` (the child-link system
/// never touches the MOTHER's sbyte1 — it only writes each CHILD's sbyte1 =
/// child_num, and uses the mother's sword1 as the link head). Legs reach it via
/// boss_get_mother_obj (the ROM leg .explode also fetches the mother right
/// after, for s_remove_child — D3:855).
fn hitcount(al: &Alien) -> u8 {
    al.sbyte1
}
fn sub_hitcount(al: &mut Alien, n: u8) {
    al.sbyte1 = al.sbyte1.saturating_sub(n); // s_sub_var bosshhitcount,#n
}

// ============================================================
// bossh_istrat — the body / mother (D3STRATS.ASM:67-585).
// ============================================================

/// bossh_istrat init block (D3:67-80): wire ptrs/data, flags, seed the phase
/// gate + boss bar, generate the family, then s_mode_change #0 and fall into
/// the tick the same frame.
pub fn bossh_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossh_strat);
    let coll = sid(g, strat_hit_flash); // s_set_alptrs ...,hitflash_istrat,...
    let exp = sid(g, bossh_explode);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.expstratptr = Some(exp);
        al.hp = BOSSH_HP; // s_set_aldata #bosshHP,#bosshAP
        al.ap = BOSSH_AP;
        al.sflags |= ASF_SHADOW; // s_set_alsflag shadow
        al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
        al.depthoffset = 1; // s_set_alvar al_depthoffset,#1
        al.sbyte3 = 1; // s_set_alvar al_sbyte3,#1 (spin rate)
        al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag nohitaffect
        al.sbyte1 = BOSSHHITCOUNT_INIT; // s_set_var bosshhitcount,#35
        al.stratstate = BossHMode::WalkOn as u8;
    }
    // s_set_bossmaxHP bosshhitcount (=35) + s_add_bossmaxHP #bosshHP (+64) = 99.
    set_bossmaxhp(g, BOSSHHITCOUNT_INIT as u16 + BOSSH_HP as u16);
    generate(g, idx); // jsr .generate
    bossh_strat(g, idx);
}

/// .generate (D3:477-492): s_make_mother + five child legs + the top, then
/// s_rotpos_allchildren to seat them.
fn generate(g: &mut Game, idx: u16) {
    // s_make_mother marks the mother; boss_attach_child_to_mother sets the flag.
    for &(child_num, _ox, _oy, _oz, local_yaw) in LEG_LAYOUT.iter() {
        if let Some(leg) = spawn_child(g, idx, SH_BOSSH_LEG, bosshleg_init) {
            if boss_attach_child_to_mother(g, idx, leg, child_num) {
                g.objs.aliens[leg as usize].childroty = local_yaw;
                g.objs.aliens[leg as usize].sbyte1 = child_num;
                seed_leg_animation(&mut g.objs.aliens[leg as usize]);
            } else {
                g.objs.free(leg);
            }
        }
    }
    if let Some(top) = spawn_child(g, idx, SH_BOSSH_TOP, bosshtop_init) {
        if !boss_attach_child_to_mother(g, idx, top, BOSSH_TOP) {
            g.objs.free(top);
        }
    }
    position_children(g, idx); // s_jsr .position
}

/// Allocate + init a child object (local copy of bosses.rs `boss2_spawn_child`
/// minus the mother-attach, which .generate does explicitly per child).
fn spawn_child(g: &mut Game, mother: u16, shape: u16, init_fn: StrategyFn) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    // Seat at the mother; the child-position pass refines it.
    let m = g.objs.aliens[mother as usize];
    {
        let al = &mut g.objs.aliens[child as usize];
        al.shape = shape;
        al.worldx = m.worldx;
        al.worldy = m.worldy;
        al.worldz = m.worldz;
    }
    init_fn(g, child);
    Some(child)
}

/// Seat each living child from its flat authored offset and local orientation.
/// The source helper applies the mother's roll, pitch, and yaw before composing
/// each child's local angles.
fn position_children(g: &mut Game, mother: u16) {
    let m = g.objs.aliens[mother as usize];
    for &(child_num, ox, oy, oz, _local_yaw) in LEG_LAYOUT.iter() {
        if let Some(leg) = boss_find_child_obj(g, mother, child_num) {
            position_child(g, leg, &m, ox, oy, oz);
        }
    }
    if let Some(top) = boss_find_child_obj(g, mother, BOSSH_TOP) {
        position_child(g, top, &m, 0, 0, 0);
    }
    if let Some(teleporter) = boss_find_child_obj(g, mother, BOSSH_TELEPORT) {
        position_child(g, teleporter, &m, 0, 0, 0);
    }
}

fn position_child(g: &mut Game, child: u16, mother: &Alien, x: i16, y: i16, z: i16) {
    let (offset_x, offset_y, offset_z) = crate::snes_trig::strat_roffs_full_scaled(
        mother.rotz,
        mother.rotx,
        mother.roty,
        x as i8,
        y as i8,
        z as i8,
        BOSSH_CHILD_POSITION_SCALE,
    );
    let object = &mut g.objs.aliens[child as usize];
    object.worldx = mother.worldx.wrapping_add(offset_x);
    object.worldy = mother.worldy.wrapping_add(offset_y);
    object.worldz = mother.worldz.wrapping_add(offset_z);
    object.rotx = mother.rotx.wrapping_add(object.childrotx);
    object.roty = mother.roty.wrapping_add(object.childroty);
    object.rotz = mother.rotz.wrapping_add(object.childrotz);
}

fn set_bossh_mode(g: &mut Game, idx: u16, mode: BossHMode) {
    g.objs.aliens[idx as usize].stratstate = mode as u8;
}

fn advance_bossh_mode(g: &mut Game, idx: u16, mode: BossHMode) {
    set_bossh_mode(g, idx, mode.next());
}

fn set_leg_modes(g: &mut Game, mother: u16, mode: BossHLegMode) {
    for child_number in BOSSH_FIRST_LEG..=BOSSH_LAST_LEG {
        if let Some(leg) = boss_find_child_obj(g, mother, child_number) {
            g.objs.aliens[leg as usize].stratstate = mode as u8;
        }
    }
}

fn legs_in_mode(g: &mut Game, mother: u16, mode: BossHLegMode) -> usize {
    let mut count = 0;
    for child_number in BOSSH_FIRST_LEG..=BOSSH_LAST_LEG {
        if let Some(leg) = boss_find_child_obj(g, mother, child_number) {
            if g.objs.aliens[leg as usize].stratstate == mode as u8 {
                count += 1;
            }
        }
    }
    count
}

fn random_branch(g: &mut Game, percent: u16) -> bool {
    let threshold = percent * BOSSH_RANDOM_PERCENT_SCALE / PERCENT_DENOMINATOR;
    u16::from(sf_random(&mut g.vars) as u8) < threshold
}

/// Complete 22-entry mother dispatch. Immediate source transitions are
/// followed in the same tick; persistent modes yield after their authored
/// movement tail.
fn bossh_strat(g: &mut Game, idx: u16) {
    for _ in 0..BOSSH_MAX_IMMEDIATE_TRANSITIONS {
        let Some(mode) = BossHMode::from_raw(g.objs.aliens[idx as usize].stratstate) else {
            set_bossh_mode(g, idx, BossHMode::AttackOscillation);
            continue;
        };
        let step = match mode {
            BossHMode::WalkOn => mode_walkon(g, idx, mode),
            BossHMode::Spin | BossHMode::WaitForRaisedLegs => bh_move2(g, idx, mode),
            BossHMode::FirstDrop | BossHMode::SecondDrop => mode_droptoground(g, idx, mode),
            BossHMode::FirstSpinUp | BossHMode::SecondSpinUp => mode_spinfaster(g, idx, mode),
            BossHMode::FirstOscillation | BossHMode::AttackOscillation => {
                mode_move_back_and_forth(g, idx, mode)
            }
            BossHMode::SetMiddlePose => {
                set_leg_modes(g, idx, BossHLegMode::MiddlePose);
                advance_bossh_mode(g, idx, mode);
                StrategyStep::Continue
            }
            BossHMode::WaitForMiddlePose => mode_wait_for_middle(g, idx, mode),
            BossHMode::ScuttleForward => mode_scuttle_forward(g, idx, mode),
            BossHMode::ScuttleBackward => mode_scuttle_backward(g, idx, mode),
            BossHMode::MakeLegsVulnerable => mode_redlegs(g, idx, mode),
            BossHMode::RetreatToFarDistance => {
                mode_move_to_distance(g, idx, mode, BOSSH_FAR_Z_DISTANCE)
            }
            BossHMode::RiseToFlightHeight => mode_float_to_height(g, idx, mode),
            BossHMode::CreateTeleporter => mode_create_teleporter(g, idx, mode),
            BossHMode::AdvanceToNearDistance => {
                mode_move_to_distance(g, idx, mode, BOSSH_NEAR_Z_DISTANCE)
            }
            BossHMode::WaitForTeleporter => mode_wait_for_teleporter(g, idx, mode),
            BossHMode::LoopDelay => mode_loop_delay(g, idx),
            BossHMode::Stand => mode_stand(g, idx, mode),
            BossHMode::Crouch => mode_crouch(g, idx),
        };
        if matches!(step, StrategyStep::Yield) {
            return;
        }
    }
    debug_assert!(false, "boss H exhausted its immediate transition budget");
}

/// .walkon (D3:131-148): slide onto the play-field — creep worldx toward centre
/// (−25/frame while worldx>=0) and worldz forward when far; advance once no
/// adjustment was needed (in position).
fn mode_walkon(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    let mut moved = false;
    if g.objs.aliens[idx as usize].worldx >= 0 {
        g.objs.aliens[idx as usize].worldx = g.objs.aliens[idx as usize]
            .worldx
            .wrapping_add(BOSSH_WALK_X_STEP);
        moved = true;
    }
    if zdist_less(g, idx, BOSSH_WALK_Z_DISTANCE) {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
            .worldz
            .wrapping_add(BOSSH_WALK_Z_STEP);
        moved = true;
    }
    if !moved {
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    bh_move(g, idx);
    StrategyStep::Yield
}

/// .droptoground (D3:348-360): fall under gravity to the authored ground
/// height; on landing, play the impact, drain the phase gate, and emit smoke.
fn mode_droptoground(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    set_leg_modes(g, idx, BossHLegMode::MoveFlat);
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let landed = falldown_yvec(
        &mut g.objs.aliens[idx as usize],
        BOSSH_FALL_BOUNCE_SHIFT,
        BOSSH_FALL_GRAVITY,
        BOSSH_GROUND_Y,
    );
    if landed {
        g.hooks.play_se(BOSSH_IMPACT_SOUND);
        sub_hitcount(&mut g.objs.aliens[idx as usize], BOSSH_IMPACT_GATE_DAMAGE);
        for _ in 0..BOSSH_SMOKE_COUNT {
            create_impact_smoke(g, idx);
        }
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    bh_move(g, idx);
    StrategyStep::Yield
}

/// .spinfaster (D3:331-344): rise (worldy −4/frame) while chasing the spin rate
/// al_sbyte3 → 20; when worldy < −400 advance. (fchase toward 20 on notdelay 2.)
fn mode_spinfaster(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if notdelay(g, BOSSH_NOTDELAY_SPIN_BITS) {
        let speed = g.objs.aliens[idx as usize].sbyte3;
        g.objs.aliens[idx as usize].sbyte3 =
            strat_chase8(speed, BOSSH_SPIN_TARGET, BOSSH_SPIN_CHASE_STEP);
    }
    let done = {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(BOSSH_SPIN_RISE_STEP);
        al.worldy < BOSSH_FLIGHT_Y
    };
    if done {
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    bh_move(g, idx);
    StrategyStep::Yield
}

/// .redlegs (D3:209-220): flip every leg's coltab to the red id_1_c, then next.
fn mode_redlegs(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    for &(child_num, ..) in LEG_LAYOUT.iter() {
        if let Some(leg) = boss_find_child_obj(g, idx, child_num) {
            g.objs.aliens[leg as usize].coltab = ID_1_C;
        }
    }
    advance_bossh_mode(g, idx, mode);
    StrategyStep::Continue
}

fn mode_wait_for_middle(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if legs_in_mode(g, idx, BossHLegMode::Middled) == LEG_LAYOUT.len() {
        set_leg_modes(g, idx, BossHLegMode::ScamperMiddle);
        advance_bossh_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        bh_move2(g, idx, mode)
    }
}

fn mode_scuttle_forward(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if zdist_less(g, idx, BOSSH_NEAR_Z_DISTANCE) {
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    let speed = g.objs.aliens[idx as usize].sbyte3;
    g.objs.aliens[idx as usize].sbyte3 = strat_chase8(
        speed,
        BOSSH_SCUTTLE_SPIN_TARGET,
        BOSSH_SCUTTLE_SPIN_CHASE_STEP,
    );
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(BOSSH_FORWARD_Z_STEP);
    bh_move2(g, idx, mode)
}

fn mode_scuttle_backward(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if !zdist_less(g, idx, BOSSH_SCUTTLE_SWITCH_DISTANCE) {
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(BOSSH_BACKWARD_Z_STEP);
    bh_move2(g, idx, mode)
}

fn mode_move_back_and_forth(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    let moving_forward = g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG2 != 0;
    if moving_forward {
        let world_y = g.objs.aliens[idx as usize].worldy;
        g.objs.aliens[idx as usize].worldy =
            strat_chase(world_y, BOSSH_FLIGHT_Y, BOSSH_HEIGHT_CHASE_STEP);
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
            .worldz
            .wrapping_add(BOSSH_OSCILLATION_Z_STEP);
        if !zdist_less(g, idx, BOSSH_FAR_Z_DISTANCE) {
            return mode_reverse_or_advance(g, idx, mode);
        }
    } else {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
            .worldz
            .wrapping_sub(BOSSH_OSCILLATION_Z_STEP);
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG3 == 0
            && zdist_less(g, idx, BOSSH_SCUTTLE_SWITCH_DISTANCE)
        {
            g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG3;
            g.objs.aliens[idx as usize].sword2 = if random_branch(g, BOSSH_RANDOM_HALF_PERCENT) {
                BOSSH_OSCILLATION_HIGH_Y
            } else {
                BOSSH_OSCILLATION_LOW_Y
            };
        }
        if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG3 != 0 {
            let target = g.objs.aliens[idx as usize].sword2;
            let current = g.objs.aliens[idx as usize].worldy;
            g.objs.aliens[idx as usize].worldy =
                strat_chase_proportional(current, target, BOSSH_HEIGHT_CHASE_SHIFT);
        }
        if zdist_less(g, idx, BOSSH_CLOSE_Z_DISTANCE) {
            return mode_reverse_or_advance(g, idx, mode);
        }
    }
    set_oscillating_x(g, idx, BOSSH_OSCILLATION_FAST_PHASE_STEP);
    bh_move(g, idx);
    StrategyStep::Yield
}

fn mode_reverse_or_advance(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if g.objs.aliens[idx as usize].sbyte2 == 1 {
        let object = &mut g.objs.aliens[idx as usize];
        object.sflags2 &= !ASF2_SFLAG2;
        object.sbyte2 = 0;
        advance_bossh_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        let object = &mut g.objs.aliens[idx as usize];
        object.sbyte2 = object.sbyte2.wrapping_add(1);
        object.sflags2 ^= ASF2_SFLAG2;
        object.sflags2 &= !ASF2_SFLAG3;
        set_oscillating_x(g, idx, BOSSH_OSCILLATION_FAST_PHASE_STEP);
        bh_move(g, idx);
        StrategyStep::Yield
    }
}

fn mode_move_to_distance(g: &mut Game, idx: u16, mode: BossHMode, distance: i16) -> StrategyStep {
    if zdist_less(g, idx, distance) {
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize]
        .worldz
        .wrapping_add(BOSSH_FORWARD_Z_STEP);
    set_oscillating_x(g, idx, BOSSH_OSCILLATION_SLOW_PHASE_STEP);
    bh_move(g, idx);
    StrategyStep::Yield
}

fn mode_float_to_height(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    let current = g.objs.aliens[idx as usize].worldy;
    let next = strat_chase_proportional(current, BOSSH_FLIGHT_Y, BOSSH_FLIGHT_CHASE_SHIFT);
    g.objs.aliens[idx as usize].worldy = next;
    if next == BOSSH_FLIGHT_Y {
        advance_bossh_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        bh_move(g, idx);
        StrategyStep::Yield
    }
}

fn mode_create_teleporter(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if let Some(teleporter) = spawn_child(g, idx, SH_BOSSH_TELEPORT, teleporter_istrat) {
        if boss_attach_child_to_mother(g, idx, teleporter, BOSSH_TELEPORT) {
            g.objs.move_obj_to_end(teleporter);
        } else {
            g.objs.free(teleporter);
        }
    }
    g.hooks.play_se(BOSSH_TELEPORT_SOUND);
    advance_bossh_mode(g, idx, mode);
    StrategyStep::Continue
}

fn mode_wait_for_teleporter(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    if boss_find_child_obj(g, idx, BOSSH_TELEPORT).is_none() {
        g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG4;
        advance_bossh_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        bh_move(g, idx);
        StrategyStep::Yield
    }
}

fn mode_loop_delay(g: &mut Game, idx: u16) -> StrategyStep {
    if g.objs.aliens[idx as usize].pbyte1 == BOSSH_LOOP_WAIT_TICKS {
        g.objs.aliens[idx as usize].pbyte1 = 0;
        set_bossh_mode(g, idx, BossHMode::AttackOscillation);
        StrategyStep::Continue
    } else {
        g.objs.aliens[idx as usize].pbyte1 = g.objs.aliens[idx as usize].pbyte1.wrapping_add(1);
        set_oscillating_x(g, idx, BOSSH_OSCILLATION_SLOW_PHASE_STEP);
        bh_move(g, idx);
        StrategyStep::Yield
    }
}

fn mode_stand(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    g.objs.aliens[idx as usize].sbyte2 = g.objs.aliens[idx as usize].sbyte2.wrapping_add(1);
    if g.objs.aliens[idx as usize].sbyte2 == BOSSH_STAND_TICKS {
        advance_bossh_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        set_leg_modes(g, idx, BossHLegMode::Stand);
        bh_move(g, idx);
        StrategyStep::Yield
    }
}

fn mode_crouch(g: &mut Game, idx: u16) -> StrategyStep {
    set_leg_modes(g, idx, BossHLegMode::Crouch);
    bh_move(g, idx);
    StrategyStep::Yield
}

fn bh_move2(g: &mut Game, idx: u16, mode: BossHMode) -> StrategyStep {
    let raised = legs_in_mode(g, idx, BossHLegMode::Waggle);
    if raised >= BOSSH_RAISED_LEGS_TO_ADVANCE {
        advance_bossh_mode(g, idx, mode);
        return StrategyStep::Continue;
    }
    if raised == 2 {
        g.objs.aliens[idx as usize].sbyte3 = BOSSH_TWO_RAISED_LEG_SPIN;
    } else if raised == 1 {
        g.objs.aliens[idx as usize].sbyte3 = BOSSH_ONE_RAISED_LEG_SPIN;
    }
    let bob = BOSSH_Y_BOB[(g.vars.gameframe & BOSSH_Y_BOB_MASK) as usize];
    g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize].worldy.wrapping_add(bob);
    set_oscillating_x(g, idx, BOSSH_OSCILLATION_SLOW_PHASE_STEP);
    bh_move(g, idx);
    StrategyStep::Yield
}

fn set_oscillating_x(g: &mut Game, idx: u16, phase_step: u8) {
    use crate::snes_trig::SINTAB;

    let phase = g.objs.aliens[idx as usize].sbyte4;
    g.objs.aliens[idx as usize].worldx = (SINTAB[phase as usize] as i16) << BOSSH_OSCILLATION_SCALE;
    g.objs.aliens[idx as usize].sbyte4 = phase.wrapping_add(phase_step);
}

/// .move tail (D3:533-584): spin the body, keep the bar fed, and gate the
/// body's vulnerability on the legs. While any leg lives the body pins al_hp =
/// bosshHP (invulnerable); once all five legs are dead it flips to the red
/// coltab, drops nohitaffect (killable), and the bar drains through al_hp.
fn bh_move(g: &mut Game, idx: u16) {
    if !random_branch(g, BOSSH_RARE_SHAKE_PERCENT) {
        let pair = if random_branch(g, BOSSH_RANDOM_HALF_PERCENT) {
            [BOSSH_LEG2, BOSSH_LEG5]
        } else {
            [BOSSH_LEG4, BOSSH_LEG1]
        };
        for child_number in pair {
            if let Some(leg) = boss_find_child_obj(g, idx, child_number) {
                g.objs.aliens[leg as usize].stratstate = BossHLegMode::BeginShake as u8;
            }
        }
    }

    let previous_half = g.objs.aliens[idx as usize].roty & BOSSH_ROTATION_HALF_MASK;
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(al.sbyte3);
    }
    let current_half = g.objs.aliens[idx as usize].roty & BOSSH_ROTATION_HALF_MASK;
    if previous_half != current_half && g.objs.aliens[idx as usize].sbyte3 >= 8 {
        g.hooks.play_se(BOSSH_GAIT_SOUND);
    }

    add_player_z(g, idx);
    if children_dead(g, idx, BOSSH_FIRST_LEG, BOSSH_LAST_LEG) {
        let al = &mut g.objs.aliens[idx as usize];
        al.coltab = ID_1_C;
        al.sflags &= !ASF_NOHITAFFECT;
        al.tx = al.tx.wrapping_add(BOSSH_VULNERABLE_TEXTURE_STEP);
    } else {
        g.objs.aliens[idx as usize].hp = BOSSH_HP;
    }
    g.objs.aliens[idx as usize].tx = g.objs.aliens[idx as usize]
        .tx
        .wrapping_add(BOSSH_TEXTURE_STEP);
    position_children(g, idx);
    add_bosshp_obj(g, idx);
    let hc = hitcount(&g.objs.aliens[idx as usize]);
    add_bosshp_val(g, hc);
}

fn create_impact_smoke(g: &mut Game, source: u16) {
    let Some(smoke) = strat_make_obj(g, BOSSH_SMOKE_SHAPE) else {
        return;
    };
    let tick = sid(g, impact_smoke_tick);
    let source_position = g.objs.aliens[source as usize];
    let texture_adjustment = ((sf_random(&mut g.vars) as u8) & BOSSH_SMOKE_X_MASK)
        .wrapping_sub(BOSSH_SMOKE_X_CENTER as u8);
    let x_adjustment =
        i16::from(sf_random(&mut g.vars) as u8 as i8).wrapping_mul(BOSSH_SMOKE_X_SCALE);
    let object = &mut g.objs.aliens[smoke as usize];
    object.worldx = source_position.worldx.wrapping_add(x_adjustment);
    object.worldy = source_position.worldy;
    object.worldz = source_position.worldz;
    object.tx = object.tx.wrapping_add(texture_adjustment);
    object.visual_kind = ObjectVisualKind::ScaledSprite;
    object.sflags |= ASF_COLLDISABLE;
    object.stratptr = Some(tick);
    object.collstratptr = None;
    object.expstratptr = None;
    init_colanim(object, 0);
}

fn impact_smoke_tick(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].colframe & ANIMATION_FRAME_MASK == BOSSH_SMOKE_FINAL_FRAME {
        g.objs.aldead = 1;
    } else {
        add_colanim_wrap(&mut g.objs.aliens[idx as usize], 1, BOSSH_SMOKE_FRAMES);
    }
}

/// Retail player-distance predicate: `|player_z - object_z| < distance`.
fn zdist_less(g: &Game, idx: u16, d: i16) -> bool {
    match player(g) {
        Some(p) => (p.worldz as i32 - g.objs.aliens[idx as usize].worldz as i32).abs() < d as i32,
        None => false,
    }
}

/// .explode (D3:413-422): kill the top, remove the teleport prop, then hand off
/// to the shared boss explosion (jml bossexplode_istrat).
fn bossh_explode(g: &mut Game, idx: u16) {
    if let Some(top) = boss_find_child_obj(g, idx, BOSSH_TOP) {
        g.objs.aliens[top as usize].hp = 0;
        g.objs.aliens[top as usize].sflags |= ASF_COLLDISABLE;
    }
    if let Some(tp) = boss_find_child_obj(g, idx, BOSSH_TELEPORT) {
        g.objs.free(tp);
    }
    strat_boss_explode_init(g, idx); // jml bossexplode_istrat
}

// ============================================================
// bosshleg_istrat — a shootable child leg (D3STRATS.ASM:589-865).
// ============================================================

/// Initialize one of the five legs with its protected HP window and authored
/// child-dependent starting animation.
pub fn bosshleg_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bosshleg_strat);
    let coll = sid(g, bosshleg_hit);
    let exp = sid(g, bosshleg_explode);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = BOSSHLEG_PROTECTED_HP;
    al.ap = BOSSHLEG_AP;
    al.depthoffset = 1;
    al.collflags |= ACF_COLLTYPE2;
    al.stratstate = BossHLegMode::Scampering as u8;
    seed_leg_animation(al);
}

fn seed_leg_animation(al: &mut Alien) {
    let frame = al.sbyte1.wrapping_sub(2) & ANIMATION_FRAME_MASK;
    al.animframe = ANIMATION_ACTIVE | frame;
}

fn animation_frame(al: &Alien) -> u8 {
    al.animframe & ANIMATION_FRAME_MASK
}

fn set_animation_frame(al: &mut Alien, frame: u8) {
    al.animframe = ANIMATION_ACTIVE | (frame & ANIMATION_FRAME_MASK);
}

fn set_leg_mode(g: &mut Game, idx: u16, mode: BossHLegMode) {
    g.objs.aliens[idx as usize].stratstate = mode as u8;
}

fn advance_leg_mode(g: &mut Game, idx: u16, mode: BossHLegMode) {
    set_leg_mode(g, idx, mode.next());
}

/// Complete walking-gait, phase-pose, and vulnerability dispatch.
fn bosshleg_strat(g: &mut Game, idx: u16) {
    for _ in 0..BOSSH_LEG_MAX_IMMEDIATE_TRANSITIONS {
        let Some(mode) = BossHLegMode::from_raw(g.objs.aliens[idx as usize].stratstate) else {
            set_leg_mode(g, idx, BossHLegMode::Scampering);
            continue;
        };
        let step = match mode {
            BossHLegMode::Scampering => leg_scamper(
                g,
                idx,
                BOSSH_LEG_SCAMPER_HIGH_FRAME,
                BOSSH_LEG_SCAMPER_LOW_FRAME,
            ),
            BossHLegMode::BeginShake => {
                g.objs.aliens[idx as usize].sflags2 &= !ASF2_SFLAG1;
                advance_leg_mode(g, idx, mode);
                StrategyStep::Continue
            }
            BossHLegMode::Shake => leg_shake(g, idx),
            BossHLegMode::Stand => leg_stand(g, idx),
            BossHLegMode::Crouch => leg_crouch(g, idx),
            BossHLegMode::Raise => leg_raise(g, idx, mode),
            BossHLegMode::Waggle => leg_waggle(g, idx),
            BossHLegMode::MoveFlat => leg_move_flat(g, idx),
            BossHLegMode::MoveToThirty => leg_move_to_thirty(g, idx),
            BossHLegMode::LowerPose => leg_pose(g, idx, mode, BOSSH_LEG_LOWER_BASE_FRAME),
            BossHLegMode::Lowered | BossHLegMode::Middled => {
                update_leg_shape(g, idx);
                StrategyStep::Yield
            }
            BossHLegMode::ScamperLow => leg_scamper(
                g,
                idx,
                BOSSH_LEG_SCAMPER_LOW_HIGH_FRAME,
                BOSSH_LEG_SCAMPER_LOW_LOW_FRAME,
            ),
            BossHLegMode::MiddlePose => leg_pose(g, idx, mode, BOSSH_LEG_MIDDLE_BASE_FRAME),
            BossHLegMode::ScamperMiddle => leg_scamper(
                g,
                idx,
                BOSSH_LEG_SCAMPER_MIDDLE_HIGH_FRAME,
                BOSSH_LEG_SCAMPER_MIDDLE_LOW_FRAME,
            ),
        };
        if matches!(step, StrategyStep::Yield) {
            return;
        }
    }
    debug_assert!(
        false,
        "boss H leg exhausted its immediate transition budget"
    );
}

fn leg_scamper(g: &mut Game, idx: u16, high: u8, low: u8) -> StrategyStep {
    let descending = g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0;
    let frame = animation_frame(&g.objs.aliens[idx as usize]);
    if descending {
        if frame == low {
            g.hooks.play_se(BOSSH_LEG_SOUND);
            g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG1;
        } else {
            add_animation_wrap(
                &mut g.objs.aliens[idx as usize],
                -1,
                BOSSH_LEG_ANIMATION_FRAMES,
            );
        }
    } else if frame == high {
        g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG1;
    } else {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            1,
            BOSSH_LEG_ANIMATION_FRAMES,
        );
    }
    if g.objs.aliens[idx as usize].hp < BOSSHLEG_RAISE_HP_THRESHOLD {
        set_leg_mode(g, idx, BossHLegMode::Raise);
    }
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_shake(g: &mut Game, idx: u16) -> StrategyStep {
    let descending = g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0;
    let frame = animation_frame(&g.objs.aliens[idx as usize]);
    if descending {
        if frame == BOSSH_LEG_SCAMPER_LOW_FRAME {
            set_leg_mode(g, idx, BossHLegMode::Scampering);
            return StrategyStep::Continue;
        }
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            BOSSH_LEG_ANIMATION_FRAMES,
        );
    } else if frame == BOSSH_LEG_SHAKE_HIGH_FRAME {
        g.objs.aliens[idx as usize].sflags2 ^= ASF2_SFLAG1;
    } else {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            1,
            BOSSH_LEG_ANIMATION_FRAMES,
        );
    }
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_stand(g: &mut Game, idx: u16) -> StrategyStep {
    let frame = animation_frame(&g.objs.aliens[idx as usize]);
    if frame != 0 {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            BOSSH_LEG_ANIMATION_FRAMES,
        );
    }
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_crouch(g: &mut Game, idx: u16) -> StrategyStep {
    let _ = add_animation_cap(&mut g.objs.aliens[idx as usize], 1, BOSSH_LEG_RAISE_LIMIT);
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_raise(g: &mut Game, idx: u16, mode: BossHLegMode) -> StrategyStep {
    if add_animation_cap(&mut g.objs.aliens[idx as usize], 1, BOSSH_LEG_RAISE_LIMIT) {
        advance_leg_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        g.objs.aliens[idx as usize].hp = BOSSHLEG_PROTECTED_HP;
        update_leg_shape(g, idx);
        StrategyStep::Yield
    }
}

fn leg_waggle(g: &mut Game, idx: u16) -> StrategyStep {
    g.objs.aliens[idx as usize].hp = BOSSHLEG_PROTECTED_HP;
    let next = if animation_frame(&g.objs.aliens[idx as usize]) == BOSSH_LEG_WAGGLE_HIGH_FRAME {
        BOSSH_LEG_WAGGLE_LOW_FRAME
    } else {
        BOSSH_LEG_WAGGLE_HIGH_FRAME
    };
    set_animation_frame(&mut g.objs.aliens[idx as usize], next);
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_move_flat(g: &mut Game, idx: u16) -> StrategyStep {
    let collision = if g.objs.aliens[idx as usize].coltab == ID_1_C {
        sid(g, bosshleg_hit)
    } else {
        g.objs.aliens[idx as usize].hp = BOSSHLEG_PROTECTED_HP;
        sid(g, defelasercol_istrat)
    };
    g.objs.aliens[idx as usize].collstratptr = Some(collision);
    let _ = add_animation_cap(&mut g.objs.aliens[idx as usize], 1, BOSSH_LEG_FLAT_LIMIT);
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_move_to_thirty(g: &mut Game, idx: u16) -> StrategyStep {
    if notdelay(g, BOSSH_NOTDELAY_LEG_BITS)
        && animation_frame(&g.objs.aliens[idx as usize]) != BOSSH_LEG_MOVE_TO_THIRTY_FRAME
    {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            BOSSH_LEG_ANIMATION_FRAMES,
        );
    }
    update_leg_shape(g, idx);
    StrategyStep::Yield
}

fn leg_pose(g: &mut Game, idx: u16, mode: BossHLegMode, base_frame: u8) -> StrategyStep {
    g.objs.aliens[idx as usize].collstratptr = Some(sid(g, bosshleg_hit));
    g.objs.aliens[idx as usize].hp = BOSSHLEG_PROTECTED_HP;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    let target =
        (g.objs.aliens[idx as usize].sbyte1 & BOSSH_LEG_CHILD_FRAME_MASK).wrapping_add(base_frame);
    if animation_frame(&g.objs.aliens[idx as usize]) == target {
        advance_leg_mode(g, idx, mode);
        StrategyStep::Continue
    } else {
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            BOSSH_LEG_ANIMATION_FRAMES,
        );
        update_leg_shape(g, idx);
        StrategyStep::Yield
    }
}

fn update_leg_shape(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].shape =
        if animation_frame(&g.objs.aliens[idx as usize]) >= BOSSH_LEG_ALTERNATE_SHAPE_FRAME {
            SH_BOSSH_LEG_ALTERNATE
        } else {
            SH_BOSSH_LEG
        };
}

/// bosshleg .hit (D3:847-849): trigse $24 + hitflash.
fn bosshleg_hit(g: &mut Game, idx: u16) {
    g.hooks.play_se(BOSSH_LEG_HIT_SOUND);
    strat_hit_flash(g, idx);
}

/// Detach a destroyed leg and preserve its authored falling arc before the
/// generic explosion lifecycle takes over.
fn bosshleg_explode(g: &mut Game, idx: u16) {
    let fall = sid(g, bosshleg_explode_fall);
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        sub_hitcount(
            &mut g.objs.aliens[mother as usize],
            BOSSH_IMPACT_GATE_DAMAGE,
        );
    }
    g.objs.divorce_family(idx);
    {
        let object = &mut g.objs.aliens[idx as usize];
        object.expstratptr = Some(fall);
        object.vel = BOSSH_LEG_FALL_SPEED;
        object.sflags |= ASF_COLLDISABLE;
        strat_gen_vecs_nvecs(object);
    }
    bosshleg_explode_fall(g, idx);
}

fn bosshleg_explode_fall(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    g.objs.aliens[idx as usize].roty = g.objs.aliens[idx as usize].roty.wrapping_add(DEG22);
    if falldown_yvec(
        &mut g.objs.aliens[idx as usize],
        BOSSH_FALL_BOUNCE_SHIFT,
        BOSSH_LEG_FALL_GRAVITY,
        BOSSH_LEG_FALL_GROUND_Y,
    ) {
        strat_explode(g, idx);
    }
}

// ============================================================
// bosshtop_istrat — the firing top (D3STRATS.ASM:868-889).
// ============================================================

/// bosshtop_istrat init (D3:868-873): shadow, hardHP (indestructible — the
/// fight is decided by the legs + body), nohitaffect.
pub fn bosshtop_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bosshtop_strat);
    let coll = sid(g, strat_hit_flash); // hitflash_istrat
    let exp = sid(g, strat_explode); // explode_istrat
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.sflags |= ASF_SHADOW; // s_set_alsflag shadow
    al.hp = HARDHP; // s_set_aldata #bosshtopHP(hardHP),#bosshtopAP
    al.ap = HARDAP;
    al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag nohitaffect
    al.collflags |= ACF_COLLTYPE2; // ROM ENEMY1
}

/// bosshtop .strat (D3:874-889): spin the child rotation; when the top faces
/// within ±deg22 of forward, fire an HPLASMA at the player on the notdelay-4
/// tick. The top's roty is refreshed to the mother's each frame by
/// position_children, so the window sweeps as the body spins.
fn bosshtop_strat(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].childroty = g.objs.aliens[idx as usize]
        .childroty
        .wrapping_add(BOSSH_TOP_SPIN_STEP);
    let yaw = g.objs.aliens[idx as usize].roty;
    let facing_forward = yaw >= 0u8.wrapping_sub(DEG22) || yaw < DEG22;
    if facing_forward && notdelay(g, BOSSH_NOTDELAY_WEAPON_BITS) {
        fire_bossh_hplasma(g, idx);
    }
}

fn fire_bossh_hplasma(g: &mut Game, idx: u16) {
    if player(g).is_none() {
        return;
    }
    const PLAYER_SLOT: u16 = 0;
    let source = g.objs.aliens[idx as usize];
    g.objs.aliens[idx as usize].rotx = source.rotx;
    g.objs.aliens[idx as usize].roty = source.roty.wrapping_add(DEG180);
    let shot = fire_hplasma(g, idx);
    g.objs.aliens[idx as usize].rotx = source.rotx;
    g.objs.aliens[idx as usize].roty = source.roty;
    let Some(shot) = shot else {
        return;
    };

    let (offset_x, offset_y, offset_z) = crate::snes_trig::strat_roffs_full_scaled(
        source.rotz,
        source.rotx,
        source.roty,
        0,
        BOSSH_HPLASMA_MUZZLE_Y,
        0,
        BOSSH_WEAPON_SCALE,
    );
    let object = &mut g.objs.aliens[shot as usize];
    object.worldx = source.worldx.wrapping_add(offset_x);
    object.worldy = source.worldy.wrapping_add(offset_y);
    object.worldz = source.worldz.wrapping_add(offset_z);
    object.ptr = PLAYER_SLOT.wrapping_add(1);
    object.count = BOSSH_HPLASMA_LIFETIME;
    object.collflags = (object.collflags & !COLLISION_TYPE_MASK) | ACF_COLLTYPE2;
}

// ============================================================
// teleporter_istrat — bossH teleport prop + bonfire (D3STRATS.ASM:892-931).
// ============================================================

/// Wrapping retail animation advance (STRATLIB.INC:180), preserving the
/// animation-active bit.
fn add_animation_wrap(al: &mut Alien, amount: i8, maxframes: u8) {
    let mut f = (al.animframe & ANIMATION_FRAME_MASK) as i8;
    f = f.wrapping_add(amount);
    if f < 0 {
        // A negative raw sum wraps back through the authored frame count.
        f = f.wrapping_add(maxframes as i8);
    }
    let mut u = (f as u8) & ANIMATION_FRAME_MASK;
    if u >= maxframes {
        u -= maxframes;
    }
    al.animframe = ANIMATION_ACTIVE | u;
}

/// Capped retail animation advance: clamp at the final frame and report the
/// endpoint transition.
fn add_animation_cap(al: &mut Alien, amount: i8, maxframes: u8) -> bool {
    let mut f = ((al.animframe & ANIMATION_FRAME_MASK) as i8).wrapping_add(amount);
    if f < 0 {
        f = f.wrapping_add(maxframes as i8);
    }
    let u = (f as u8) & ANIMATION_FRAME_MASK;
    if u >= maxframes {
        al.animframe = ANIMATION_ACTIVE | (maxframes - 1);
        true
    } else {
        al.animframe = ANIMATION_ACTIVE | u;
        false
    }
}

/// Retail `teleporter_istrat` (D3STRATS.ASM:892) falls through into its first
/// tick.
pub fn teleporter_istrat(g: &mut Game, idx: u16) {
    let tick = sid(g, teleporter_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = SH_BOSSH_TELEPORT;
        al.animframe = ANIMATION_ACTIVE;
        al.hp = HARD_HP;
        al.ap = HARD_AP;
        al.sflags |= ASF_COLLDISABLE;
        al.collflags |= ACF_COLLTYPE2;
        al.stratptr = Some(tick);
        al.sbyte2 = TELEPORTER_START_DELAY;
    }
    teleporter_strat(g, idx);
}

/// Retail teleporter tick: rise with two bonfire pulses, then retract and
/// remove once its parent requests shutdown.
pub fn teleporter_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1 != 0 {
        if (g.objs.aliens[idx as usize].animframe & ANIMATION_FRAME_MASK) == 0 {
            g.objs.aldead = 1; // .nomore2
            return;
        }
        add_animation_wrap(
            &mut g.objs.aliens[idx as usize],
            -1,
            TELEPORTER_RETRACT_FRAMES,
        );
        teleporter_move(g, idx);
        return;
    }

    let sb = g.objs.aliens[idx as usize].sbyte2;
    if sb == TELEPORTER_FIRST_BONFIRE_TICK || sb == TELEPORTER_FINAL_BONFIRE_TICK {
        let _ = fire_bonfire(g, idx);
    }

    if g.objs.aliens[idx as usize].sbyte2 == 0 {
        if add_animation_cap(
            &mut g.objs.aliens[idx as usize],
            1,
            TELEPORTER_COLLAPSE_FRAMES,
        ) {
            g.objs.aldead = 1;
            return;
        }
        g.objs.aliens[idx as usize].ty = g.objs.aliens[idx as usize]
            .ty
            .wrapping_sub(TELEPORTER_TEXTURE_RISE_STEP);
        teleporter_move(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte2 -= 1;

    let _ = add_animation_cap(&mut g.objs.aliens[idx as usize], 1, TELEPORTER_RISE_FRAMES);
    g.objs.aliens[idx as usize].ty = g.objs.aliens[idx as usize]
        .ty
        .wrapping_sub(TELEPORTER_TEXTURE_RISE_STEP);
    teleporter_move(g, idx);
}

fn teleporter_move(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    let al = &mut g.objs.aliens[idx as usize];
    al.rotx = 0;
    al.roty = 0;
    al.rotz = 0;
}

// ============================================================
// Registration.
// ============================================================

/// Register bossH under its typed authored-map identity.
pub fn register(world: &mut World) {
    let id = wsid(world, bossh_init);
    world.register_direct_strategy(STRATEGY_BOSSH, id);
    // Pre-register the child strategies so their registry ids exist even before
    // a mother spawns them (mirrors how sids resolve at runtime).
    let _ = wsid(world, bosshleg_init);
    let _ = wsid(world, bosshtop_init);
    let _ = wsid(world, teleporter_istrat);
}
