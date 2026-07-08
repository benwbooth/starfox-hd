//! bossH — the "gggy" walking/legged spider boss (RIIR port, ASM ground truth).
//!
//! Four strategy entities from `reference/ultrastarfox/SF/STRAT/D3STRATS.ASM`
//! (lines 34-931): `bossh_istrat` (the body/mother), `bosshleg_istrat` (five
//! child legs), `bosshtop_istrat` (the firing top), and `teleporter_istrat`
//! (a late-game cosmetic teleport prop). There is NO C-oracle counterpart
//! (`strat_boss*.c` never ported bossH) and NO sf-oracle differential fixture,
//! so every cite below is to the 65816 source. The Andross module
//! (`bossb.rs`) is the structural template (child-linked multi-part boss in a
//! standalone module); private `bosses.rs` helpers are replicated locally per
//! the lane rules.
//!
//! ── ISTRAT / MAP WIRING ────────────────────────────────────────────────────
//! bossH has NO `def_Istrat` row in ISTRATS.ASM (grep confirms: neither
//! `bossh`, `bosshleg`, `bosshtop`, nor `teleport` appears). It is placed by a
//! DIRECT strategy-address reference: `MAP1_4.ASM:217`
//!   `mapobj 0000,2000,-600,1000,boss_h_0,bossh_istrat`
//! i.e. the assembler emits the 24-bit address of `bossh_istrat` into the map
//! bytecode (exactly like `bossB`'s synthetic-address placement in MAP1_5).
//! We therefore register `bossh_istrat` under a synthetic strategy address
//! `STRAT_ADDR_BOSSH` (0x060011, a free slot in the 0x0600xx boss block —
//! 0x0F=BOSSB, 0x10=BOSSF, 0x14/15/16=BOSS8 group are taken; see
//! table.rs:210). The legs/top are spawned in-code by the mother, so they need
//! no address.
//!
//! MAP-WIRING CAVEAT (reported to caller): sf-map's ported MAP1_4
//! (`route1/level1_4.rs:312`) currently PLACEHOLDER-wires the boss_h_0 mapobj
//! to `lc::IS_BOSS2` (the Attack-Carrier istrat) rather than to bossH — bossH
//! had no ported strategy when that map landed. So bossH is registered and
//! resolvable here, but will not spawn LIVE from MAP1_4 until that one line is
//! rewired to place the boss via `STRAT_ADDR_BOSSH` (a `mapnobj(...,0x060011)`
//! call). This mirrors the bossBrob "stub-pending" situation — the strategy is
//! ready; the map hookup is a one-line follow-up outside this lane's files.
//!
//! ── FIDELITY / SCOPE ───────────────────────────────────────────────────────
//! Per task guidance a PLAYABLE, KILLABLE bossH beats an exhaustive one.
//! FULLY PORTED (ASM-faithful, cited inline):
//!   • init + HP-bar model (s_set_var bosshhitcount = 5*2+5*5 = 35;
//!     s_set_bossmaxHP bosshhitcount + s_add_bossmaxHP #bosshHP = 99;
//!     per-frame s_add_bossHP x,al_hp + s_add_bossHP bosshhitcount);
//!   • the `.generate` mother + five child legs + top spawn (child slots
//!     1/3/5/2/4 = leg1..5, slot 6 = top) at their arrangement offsets;
//!   • the `bosshhitcount` PHASE GATE: −5 per scripted `droptoground`
//!     (D3:356) and −5 per leg destroyed (D3:853);
//!   • the `.move` tail's leg-dead → vulnerable transition
//!     (s_jmp_childrendead x,#1,#5 → drop nohitaffect + red coltab; else pin
//!     al_hp = bosshHP so the body is invulnerable while any leg lives);
//!   • the leg = shootable child that on death subtracts the hitcount gate,
//!     detaches from the mother, and explodes;
//!   • the top's roty-window fire gate (fires when facing ±deg22 forward on the
//!     notdelay-4 tick);
//!   • death → mother `.explode` (kill top + remove teleport → boss explosion).
//! SCOPED OUT (inline notes at each site): the deep leg sub-animation pose
//! machine (bhl_scampering/waggle/lowerpose/middlepose/scamper2/3/moveto30/
//! shakealeg — the walking-gait frame juggling, D3:602-844), the teleport
//! prop + bonfire attack (teleporter_istrat/fire_bonfire, D3:892-978), the
//! smoke puffs (`.createsmoke`), the two-shape leg swap (boss_h_1/boss_h_1a),
//! and the exotic HPLASMA muzzle mesh (fired through the ported projectile
//! helper). The choreography of the 22-entry mother mode table is condensed to
//! its load-bearing phases (walkon → drop → rise/spin → attack-loop) with the
//! ROM mode indices preserved for the phase gate; the pose/scuttle/teleport
//! modes advance on a timer rather than driving leg poses.

#![allow(dead_code)]

use sf_game::alien::{
    Alien, StratId, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::obj::strat_init_obj_vars;
use sf_game::vars::COLLTYPE_ENEMY1;
use sf_game::world::World;

use crate::common::{
    strat_angle_xz as angle_xz, strat_apply_velocity as apply_velocity,
    strat_spawn_projectile as spawn_projectile,
};
use crate::enemy_a::{
    add_player_z, boss_apply_yaw_offset, boss_attach_child_to_mother, boss_clear_child_link,
    boss_find_child_obj, boss_get_mother_obj, boss_yaw_offset_pos, player,
    strat_boss_explode_init, strat_explode, strat_hit_flash, strat_pitch_toward, ASF3_CHILDOBJ,
};

// ============================================================
// Constants — verbatim D3STRATS.ASM equs + VARS.INC / STRATEQU.INC.
// ============================================================
const BOSSH_HP: u8 = 64; // D3STRATS.ASM:34 bosshHP
const BOSSH_AP: u8 = 4; // D3STRATS.ASM:35 bosshAP
const BOSSHLEG_HP: u8 = 10; // D3STRATS.ASM:36 bosshlegHP
const BOSSHLEG_AP: u8 = 4; // D3STRATS.ASM:37 bosshlegAP
const HARDHP: u8 = 0xFF; // STRATEQU.INC:68 hardHP == -1 (bosshtopHP)
const HARDAP: u8 = 8; // STRATEQU.INC:66 hardAP (bosshtopAP)

/// s_set_var bosshhitcount,#5*2+5*5 (D3STRATS.ASM:76): the phase gate seed —
/// 5 per scripted drop ×2 + 5 per leg ×5 = 35.
const BOSSHHITCOUNT_INIT: u8 = 5 * 2 + 5 * 5;

const DEG22: u8 = 16; // VARS.INC:15 deg22 = deg360/16
const DEG180: u8 = 128; // VARS.INC:12

/// id_1_c coltab (red-hot palette the body/legs flip to when vulnerable). The
/// meshes aren't wired, so the exact value is cosmetic; a nonzero marker keeps
/// the ASM semantics observable.
const ID_1_C: u16 = 0x0001;

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

/// Per-leg (child_num, offx, offy, offz, roty) from the five
/// s_make_childobjrotpos calls (D3:479-487). Angles: deg72=51 deg144=102
/// deg216=153 deg288=204 deg180=128 (deg360=256), all wrapped to u8.
/// leg1 roty=-deg180=128; leg2 -deg72-deg180=-179→77; leg3 -deg144-deg180=
/// -230→26; leg4 -deg216+deg180=-25→231; leg5 -deg288+deg180=-76→180.
const LEG_LAYOUT: [(u8, i16, i16, i16, u8); 5] = [
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
const M_WALKON: u8 = 0;
const M_SPIN: u8 = 1;
const M_DROP1: u8 = 2;
const M_SPINFAST1: u8 = 3;
const M_MOVEBF1: u8 = 4;
const M_DROP2: u8 = 10;
const M_REDLEGS: u8 = 11;
const M_SPINFAST2: u8 = 12;
const M_WAITLOOP: u8 = 19;
const M_LAST_MODE: u8 = 21; // .crouch (last table entry)
/// s_mode_entry .movebackandforth,bh_looptohere (D3:98) — the attack loop head.
const BH_LOOPTOHERE: u8 = 13;

// ============================================================
// Registry / synthetic address.
// ============================================================

/// The synthetic strategy address MAP1_4 uses for `bossh_istrat` (see module
/// doc). Free slot in the 0x0600xx boss block.
pub const STRAT_ADDR_BOSSH: u32 = 0x060011;

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

/// s_add_bossHP x,al_hp (STRATLIB.INC:562, obj/alvar form): m_bossHP += al_hp.
/// m_bossHP is the per-frame accumulator (init_strats zeroes it); HUD bar =
/// m_bossHP / m_bossmaxHP.
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

/// s_jmp_childrendead x,#begin,#end (STRATLIB.INC:801): TRUE when NO child in
/// [begin,end] is still linked/alive. Local copy of bosses.rs `wm_children_dead`.
fn children_dead(g: &mut Game, mother: u16, begin: u8, end: u8) -> bool {
    for n in begin..=end {
        if boss_find_child_obj(g, mother, n).is_some() {
            return false;
        }
    }
    true
}

/// s_falldown_Yvec x,4,#gravity,#ground (STRATMAC.INC:1813). Twin of
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
        v = 0; // cmp #-5 / bcc / lda #0 clamp
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
        al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype ENEMY1
        al.depthoffset = 1; // s_set_alvar al_depthoffset,#1
        al.sbyte3 = 1; // s_set_alvar al_sbyte3,#1 (spin rate)
        al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag nohitaffect
        al.sbyte1 = BOSSHHITCOUNT_INIT; // s_set_var bosshhitcount,#35
        al.stratstate = M_WALKON; // s_mode_change x,#0
        al.count = 0xFF; // mode-entry sentinel (see mode_frames)
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
    for &(child_num, _ox, _oy, _oz, roty) in LEG_LAYOUT.iter() {
        if let Some(leg) = spawn_child(g, idx, bosshleg_init) {
            if boss_attach_child_to_mother(g, idx, leg, child_num) {
                // s_make_childobjrotpos seeds the leg's arrangement facing.
                g.objs.aliens[leg as usize].roty = roty;
                g.objs.aliens[leg as usize].sbyte1 = child_num; // (link also sets this)
            } else {
                g.objs.free(leg);
            }
        }
    }
    if let Some(top) = spawn_child(g, idx, bosshtop_init) {
        if !boss_attach_child_to_mother(g, idx, top, BOSSH_TOP) {
            g.objs.free(top);
        }
    }
    position_children(g, idx); // s_jsr .position
}

/// Allocate + init a child object (local copy of bosses.rs `boss2_spawn_child`
/// minus the mother-attach, which .generate does explicitly per child).
fn spawn_child(g: &mut Game, mother: u16, init_fn: StrategyFn) -> Option<u16> {
    let child = g.objs.alloc()?;
    strat_init_obj_vars(&mut g.objs.aliens[child as usize]);
    // s_copy_pos y,x — seat at the mother; rotpos will refine it.
    let m = g.objs.aliens[mother as usize];
    {
        let al = &mut g.objs.aliens[child as usize];
        al.worldx = m.worldx;
        al.worldy = m.worldy;
        al.worldz = m.worldz;
    }
    init_fn(g, child);
    Some(child)
}

/// .position / s_rotpos_allchildren (D3:494-496): seat each living child at the
/// mother's pos + yaw-rotated local offset. Legs keep their arrangement facing
/// (position-only); the top inherits the mother's rotation so its fire-window
/// gate sweeps with the body. Full 3-axis child rotpos is scoped to yaw.
fn position_children(g: &mut Game, mother: u16) {
    let m = g.objs.aliens[mother as usize];
    for &(child_num, ox, oy, oz, _roty) in LEG_LAYOUT.iter() {
        if let Some(leg) = boss_find_child_obj(g, mother, child_num) {
            boss_yaw_offset_pos(g, leg, &m, ox, oy, oz);
        }
    }
    if let Some(top) = boss_find_child_obj(g, mother, BOSSH_TOP) {
        boss_apply_yaw_offset(g, top, &m, 0, 0, 0);
    }
}

/// mode-entry frame counter: returns frames-in-mode, resetting on mode change.
/// Uses the mother's spare al_count (mode id) + al_count1 (elapsed). Lets the
/// condensed choreography modes advance on a timer without stranding.
fn mode_frames(al: &mut Alien) -> u8 {
    if al.count != al.stratstate {
        al.count = al.stratstate;
        al.count1 = 0;
    } else {
        al.count1 = al.count1.wrapping_add(1);
    }
    al.count1
}

fn nxtmode(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.stratstate = al.stratstate.wrapping_add(1); // s_mode_change x,+1
}

/// bossh_strat mode dispatch (D3:82-119). Faithful bodies for the load-bearing
/// phases; condensed timer-advance for the pose/scuttle/teleport choreography.
fn bossh_strat(g: &mut Game, idx: u16) {
    let mode = g.objs.aliens[idx as usize].stratstate;
    match mode {
        M_WALKON => mode_walkon(g, idx),
        M_DROP1 | M_DROP2 => mode_droptoground(g, idx),
        M_SPINFAST1 | M_SPINFAST2 => mode_spinfaster(g, idx),
        M_REDLEGS => mode_redlegs(g, idx),
        M_WAITLOOP => {
            // .waitabitthenloop (D3:152-160): hold, then jump back to the loop
            // head (bh_looptohere). Condensed: after ~30 frames, loop.
            if mode_frames(&mut g.objs.aliens[idx as usize]) >= 30 {
                g.objs.aliens[idx as usize].stratstate = BH_LOOPTOHERE;
            }
            bh_move(g, idx);
        }
        m if m > M_LAST_MODE => {
            // past the table -> restart the attack loop.
            g.objs.aliens[idx as usize].stratstate = BH_LOOPTOHERE;
            bh_move(g, idx);
        }
        _ => {
            // Condensed choreography modes (spin / middlestage / waitformiddle /
            // scuttle*/move2 / movebackandforth / moveto*/floatto/teleport /
            // stand / crouch): cruise via bh_move2 then advance after a beat.
            // SCOPED: the per-mode leg-pose driving (.setchildmode) is omitted.
            if mode_frames(&mut g.objs.aliens[idx as usize]) >= 45 {
                nxtmode(g, idx);
            }
            bh_move2(g, idx);
        }
    }
}

/// .walkon (D3:131-148): slide onto the play-field — creep worldx toward centre
/// (−25/frame while worldx>=0) and worldz forward when far; advance once no
/// adjustment was needed (in position).
fn mode_walkon(g: &mut Game, idx: u16) {
    let mut moved = false;
    // s_cmp_alvar al_worldx,#0 / s_bmi .noadd / s_add_alvar al_worldx,#-25.
    if g.objs.aliens[idx as usize].worldx >= 0 {
        g.objs.aliens[idx as usize].worldx = g.objs.aliens[idx as usize].worldx.wrapping_add(-25);
        moved = true;
    }
    // lda #2500 / jsr d3zdistless / bcc skip / s_add_alvar al_worldz,#20.
    if zdist_less(g, idx, 2500) {
        g.objs.aliens[idx as usize].worldz = g.objs.aliens[idx as usize].worldz.wrapping_add(20);
        moved = true;
    }
    if !moved {
        nxtmode(g, idx); // s_beq .nxtmode (in position)
    }
    bh_move(g, idx); // jmp .move
}

/// .droptoground (D3:348-360): fall under gravity to y=-80; on landing, the
/// phase gate drops 5 (trigse $8e + smoke are scoped) and we advance.
fn mode_droptoground(g: &mut Game, idx: u16) {
    // s_add_vecs2pos x (apply current velocity, incl. vy, to position).
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    // s_falldown_Yvec x,4,#8,#-80,.snd.
    let landed = falldown_yvec(&mut g.objs.aliens[idx as usize], 4, 8, -80);
    if landed {
        // .snd: s_sub_var bosshhitcount,#5 (+ trigse $8e + 3× smoke, scoped).
        sub_hitcount(&mut g.objs.aliens[idx as usize], 5);
        nxtmode(g, idx);
    }
    bh_move(g, idx);
}

/// .spinfaster (D3:331-344): rise (worldy −4/frame) while chasing the spin rate
/// al_sbyte3 → 20; when worldy < −400 advance. (fchase toward 20 on notdelay 2.)
fn mode_spinfaster(g: &mut Game, idx: u16) {
    if !notdelay(g, 1) {
        // s_jmp_notdelay 2,.nochase inverted: chase on the 1-in-4 tick.
    } else {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte3 < 20 {
            al.sbyte3 = al.sbyte3.wrapping_add(1); // s_fchase al_sbyte3,#20,1
        }
    }
    let done = {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(-4); // s_add_alvar al_worldy,#-4
        al.worldy < -400 // s_cmp_alvar al_worldy,#-400 / s_bmi .nxtmode
    };
    if done {
        nxtmode(g, idx);
    }
    bh_move(g, idx);
}

/// .redlegs (D3:209-220): flip every leg's coltab to the red id_1_c, then next.
fn mode_redlegs(g: &mut Game, idx: u16) {
    for &(child_num, ..) in LEG_LAYOUT.iter() {
        if let Some(leg) = boss_find_child_obj(g, idx, child_num) {
            g.objs.aliens[leg as usize].coltab = ID_1_C;
        }
    }
    nxtmode(g, idx);
    bh_move(g, idx);
}

/// .move2 → .move3 → .move (D3:498-584): the shared tick tail with the y-bob
/// wobble. Condensed: skip the leg-waggle count (drives leg poses only) and do
/// the gameframe&3 y-bob, then fall into bh_move.
fn bh_move2(g: &mut Game, idx: u16) {
    // ytab1 (D3:468-472): -15,-5,5,15 indexed by gameframe&3.
    const YTAB1: [i16; 4] = [-15, -5, 5, 15];
    let bob = YTAB1[(g.vars.gameframe & 3) as usize];
    g.objs.aliens[idx as usize].worldy = g.objs.aliens[idx as usize].worldy.wrapping_add(bob);
    bh_move(g, idx);
}

/// .move tail (D3:533-584): spin the body, keep the bar fed, and gate the
/// body's vulnerability on the legs. While any leg lives the body pins al_hp =
/// bosshHP (invulnerable); once all five legs are dead it flips to the red
/// coltab, drops nohitaffect (killable), and the bar drains through al_hp.
fn bh_move(g: &mut Game, idx: u16) {
    // s_add_alvars al_roty,al_sbyte3 (D3:553): sweep the heading (top fire gate)
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(al.sbyte3);
    }
    add_player_z(g, idx); // s_add_playerz x
    // s_jmp_childrendead x,#1,#5,.setcoltab.
    if children_dead(g, idx, 1, 5) {
        let al = &mut g.objs.aliens[idx as usize];
        al.coltab = ID_1_C; // s_set_coltab x,#id_1_c
        al.sflags &= !ASF_NOHITAFFECT; // s_clr_alsflag nohitaffect (killable)
        al.tx = al.tx.wrapping_add(10);
    } else {
        g.objs.aliens[idx as usize].hp = BOSSH_HP; // s_set_alvar al_hp,#bosshHP (pin)
    }
    g.objs.aliens[idx as usize].tx = g.objs.aliens[idx as usize].tx.wrapping_add(5);
    position_children(g, idx); // jsr .position
    // s_add_bossHP x,al_hp ; s_add_bossHP bosshhitcount.
    add_bosshp_obj(g, idx);
    let hc = hitcount(&g.objs.aliens[idx as usize]);
    add_bosshp_val(g, hc);
}

/// s_jmp_Zdistless x,y,#d — |dz| < d.
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
        g.objs.aliens[top as usize].hp = 0; // s_kill_obj y
        g.objs.aliens[top as usize].sflags |= ASF_COLLDISABLE;
    }
    if let Some(tp) = boss_find_child_obj(g, idx, BOSSH_TELEPORT) {
        g.objs.free(tp); // s_remove_obj y
    }
    strat_boss_explode_init(g, idx); // jml bossexplode_istrat
}

// ============================================================
// bosshleg_istrat — a shootable child leg (D3STRATS.ASM:589-865).
// ============================================================

/// bosshleg_istrat init (D3:589-601). ROM data is bosshlegHP+64 (the +64 is a
/// pose-window invulnerability offset re-pinned across the leg's animation
/// states — that sub-anim machine is SCOPED). The ported leg carries the
/// effective bosshlegHP so it is plainly shootable and the fight resolves.
pub fn bosshleg_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bosshleg_strat);
    let coll = sid(g, bosshleg_hit); // .hit
    let exp = sid(g, bosshleg_explode); // .explode
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.expstratptr = Some(exp);
    al.hp = BOSSHLEG_HP; // effective HP (see note; ROM = bosshlegHP+64)
    al.ap = BOSSHLEG_AP;
    al.depthoffset = 1; // s_set_alvar al_depthoffset,#1
    al.collflags |= COLLTYPE_ENEMY1;
}

/// bosshleg .strat (D3:602-844): the walking-gait pose machine. SCOPED to a
/// static hold — the leg simply exists and stays shootable; its pose/anim
/// frames (scampering/waggle/lower/middle/scamper2/3/moveto30/shakealeg) are
/// cosmetic and driven by the mother's .setchildmode, which is also scoped.
fn bosshleg_strat(_g: &mut Game, _idx: u16) {
    // s_end_strat — nothing to advance in the condensed leg.
}

/// bosshleg .hit (D3:847-849): trigse $24 + hitflash.
fn bosshleg_hit(g: &mut Game, idx: u16) {
    strat_hit_flash(g, idx);
}

/// bosshleg .explode (D3:852-865): the phase gate drops 5, the leg detaches
/// from the mother (s_remove_child) and falls away exploding. Condensed: the
/// falling-arc frames (s_gen_vecs / s_falldown_Yvec → explode_istrat) collapse
/// to the shared explosion; the −5 gate + detach (so childrendead counts it) is
/// exact.
fn bosshleg_explode(g: &mut Game, idx: u16) {
    // s_sub_var bosshhitcount,#5 — reach the mother's gate byte.
    if let Some(mother) = boss_get_mother_obj(g, idx) {
        sub_hitcount(&mut g.objs.aliens[mother as usize], 5);
    }
    // s_set_objtobemother / s_remove_child x,y — unlink so children_dead sees
    // it gone even before the pool frees the object.
    boss_clear_child_link(g, idx);
    g.objs.aliens[idx as usize].sflags3 &= !ASF3_CHILDOBJ;
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE; // s_set_alsflag colldisable
    strat_explode(g, idx); // -> the falling explode burst (condensed)
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
    al.collflags |= COLLTYPE_ENEMY1;
}

/// bosshtop .strat (D3:874-889): spin the child rotation; when the top faces
/// within ±deg22 of forward, fire an HPLASMA at the player on the notdelay-4
/// tick. The top's roty is refreshed to the mother's each frame by
/// position_children, so the window sweeps as the body spins.
fn bosshtop_strat(g: &mut Game, idx: u16) {
    // s_add_alvar al_childroty,#5 (cosmetic barrel spin).
    g.objs.aliens[idx as usize].childroty = g.objs.aliens[idx as usize].childroty.wrapping_add(5);
    // s_cmp_alvar al_roty,#-deg22 / bcs .fire ; s_cmp_alvar al_roty,#deg22 /
    // bcs .nofire — fire when roty ∈ [256-deg22 .. 255] ∪ [0 .. deg22-1].
    let ry = g.objs.aliens[idx as usize].roty;
    let facing_forward = ry >= (0u8.wrapping_sub(DEG22)) || ry < DEG22;
    if facing_forward && notdelay(g, 4) {
        // s_fire_weapon x,HPLASMA aimed at the player (muzzle mesh scoped).
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            let _ = spawn_projectile(g, Some(idx), 0, -50, 0, pitch, yaw, 60, 255, 8, 0x40);
        }
    }
}

// ============================================================
// Registration.
// ============================================================

/// Register bossH under its synthetic map address (see module doc for the
/// MAP1_4 rewire caveat). Called from table::register_all after the other
/// register() calls.
pub fn register(world: &mut World) {
    let id = wsid(world, bossh_init);
    world.register_strategy_address(STRAT_ADDR_BOSSH, id);
    // Pre-register the child strategies so their registry ids exist even before
    // a mother spawns them (mirrors how sids resolve at runtime).
    let _ = wsid(world, bosshleg_init);
    let _ = wsid(world, bosshtop_init);
}
