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
//!     `MAP1_6A.ASM:292` (`boss_b_1,bossBrob_Istrat`), which sf-map
//!     `route1/level1_6.rs:78` currently STUBS (`map1_6a` = mapwait/maprts,
//!     content not yet ported). So bossBrob's istrat row is populated and
//!     resolvable by flat/synthetic id, but no live map spawns it yet — it
//!     will light up when MAP1_6A is ported. Reported to caller.
//!
//! FIDELITY / SCOPE (per task guidance — a playable, killable Andross beats a
//! half-written exhaustive one). FULLY PORTED: init + HP bar (s_set_bossmaxHP /
//! s_add_bossHP accumulator), the face's approach + dodge attack (quadrant
//! move-table + laser/homing fire) + HP-threshold phase-down (spin -> spinend
//! terminal drain) + escape-on-death (GF_BOSSDEAD); the robot's approach +
//! split (spawns the shootable face/hand parts that carry the boss bar) + the
//! 8-state attack rotation (bossBrobnextstate) firing each state + the Ouch
//! damage-reaction knockback/counter-fire + death -> explode.
//! SCOPED OUT (honest inline notes at each site): the cosmetic entity-image
//! trail (bossBent / bossBspinend multi-entity juggling), the scream sub-anim
//! variants, the per-frame jump-arc animation frames (s_add_anim cosmetics),
//! and the robot's full multi-FORM transformation chain (bossBrobchg ->
//! chg2/3/4 -> walking-legs bossBrobstart -> undead) — the ported robot fights
//! in its floating form and explodes when killed rather than morphing to legs.
//! Exotic projectile types (HMISSILE1 / BOSSHMISSILE1 / CHICKHMISSILE1 / HPLASMA)
//! are fired through the nearest ported weapon helper (relslowElaser[home]) —
//! the boss still fires damaging shots at the player; the muzzle mesh/homing
//! nuance is the only loss.

#![allow(dead_code)]

use sf_game::alien::{
    Alien, StratId, ACF_COLLTYPE4, ASF_COLLDISABLE, ASF_NOHITAFFECT, ASF_SHADOW, ATZREMOVE,
};
use sf_game::game::{Game, StrategyFn};
use sf_game::vars::GF_BOSSDEAD;
use sf_game::world::World;

use crate::common::{
    strat_angle_xz as angle_xz, strat_apply_velocity as apply_velocity,
    strat_gen_vecs_3d as gen_vecs_3d, strat_make_obj as make_obj,
    strat_spawn_projectile as spawn_projectile, strat_speed_to as speed_to,
};
use crate::enemy_a::{
    achase_angle, add_player_z, boss_attach_child_to_mother, boss_keeprel_to_player, copy_pos,
    currentlevel, ea_random, player, strat_aim_3d, strat_boss_explode_init, strat_hit_flash,
    strat_pitch_toward,
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

const DEG180: u8 = 128; // VARS.INC:12
const DEG90: u8 = 64; // VARS.INC:13
const DEG45: u8 = 32; // VARS.INC:14
const DEG11: u8 = 8; // VARS.INC:16 deg11 = deg360/32

const SPACE_VIEWCY: i16 = -60; // STRATEQU.INC:494

// al_sflags2 bits (STRATEQU.INC:896-915 byte2 layout).
const ASF2_SFLAG1: u8 = 0x10; // "image 1"
const ASF2_SFLAG2: u8 = 0x20; // "image 2"
const ASF2_SFLAG3: u8 = 0x40;
const ASF2_SFLAG4: u8 = 0x80;

// al_hitflags zone bits (VARS.INC:167-169 HF1..HF3).
const HF1: u8 = 1; // top
const HF2: u8 = 2; // left
const HF3: u8 = 4; // right

// enemy collision-type bits (COLLTYPE_*).
const COLLTYPE_ENEMY2: u8 = 0x02;
const COLLTYPE_ENEMYWEAP: u8 = 0x04;

/// boss_b_1 face shape proxy (sf-map `SH_BOSS_B_1_PROXY` = NULLSHAPE; meshes
/// not wired). The robot uses the same block. Value is cosmetic for the sim.
const SH_BOSS_B_1: u16 = 0;

/// STRAT_ADDR_BOSSB — the synthetic strategy address MAP1_5 uses for the face
/// (sf-map `route1/level1_5.rs:53`). Registered in `register()`.
pub const STRAT_ADDR_BOSSB: u32 = 0x06000F;

/// ISTRATS.ASM def_Istrat indices (MACRO-counted).
pub const IS_BOSSB: usize = 115;
pub const IS_BOSSBROB: usize = 118;

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

/// bossBrange_srou (GB3STRAT.ASM:1817-1826): the 2D range from the boss's world
/// XY to a target XY, via `xydiffs_abs_l` — the same scaled Euclidean magnitude
/// as Strat_DistXZ but over (dx,dy) instead of (dx,dz) (COLDET.ASM). Wrapping
/// 16-bit to match ROM.
fn range_xy(worldx: i16, worldy: i16, tx: i16, ty: i16) -> i16 {
    let mut x1 = tx.wrapping_sub(worldx);
    if x1 < 0 {
        x1 = x1.wrapping_neg();
    }
    let mut y1 = ty.wrapping_sub(worldy);
    if y1 < 0 {
        y1 = y1.wrapping_neg();
    }
    x1 >>= 1;
    y1 >>= 1;
    let range = (y1.wrapping_add(x1)).wrapping_shl(1);
    let m = if y1 < x1 { x1 } else { y1 };
    let t = m.wrapping_add(range);
    let acc = (t >> 1).wrapping_add(t.wrapping_shl(2));
    ((acc >> 1) >> 1) >> 1
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

// -------- fire helpers (exotic types -> nearest ported weapon; see module doc)

/// RELSLOWELASER at self, aimed pitch/yaw with a ±spread (s_weapon_rndrot m,m =
/// per-axis (rnd&(2m-1))-m; STRATMAC.INC). Straight enemy laser.
fn fire_relslowlaser(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let speed = if currentlevel(g) == 1 { 48 } else { 60 };
    let _ = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        speed,
        40,
        2,
        ACF_COLLTYPE4,
    );
}
/// RELSLOWELASERHOME / HMISSILE1 / BOSSHMISSILE1 / CHICKHMISSILE1 — homing shot
/// approximation (see module doc). Uses the relslowElaserHome projectile.
fn fire_home(g: &mut Game, idx: u16, pitch: u8, yaw: u8) {
    let speed = if currentlevel(g) == 1 { 48 } else { 60 };
    let Some(shot) = spawn_projectile(
        g,
        Some(idx),
        0,
        0,
        0,
        pitch,
        yaw,
        speed,
        40,
        2,
        ACF_COLLTYPE4,
    ) else {
        return;
    };
    // Tag as a missile so its type reads distinct from a bare laser.
    g.objs.aliens[shot as usize].type_ |= ATZREMOVE;
}

/// Aim yaw+pitch at the player then fire a randomly-spread laser (the common
/// `.fire` tail shared by dodge / fireP1 / rndpos). Returns after firing.
fn aim_and_fire_laser(g: &mut Game, idx: u16, spread: u8) {
    if let Some(p) = player(g) {
        let me = g.objs.aliens[idx as usize];
        let yaw = angle_xz(&me, &p);
        let pitch = strat_pitch_toward(&me, &p);
        let m = spread.max(1);
        let dyaw = (ea_random(g) as u8 % (2 * m + 1)).wrapping_sub(m);
        let dp = (ea_random(g) as u8 % (2 * m + 1)).wrapping_sub(m);
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

/// bossB_Istrat (GB3STRAT.ASM:1252-1268): wire data/ptrs, pose, then fall into
/// bossB_strat the same tick (no s_end between the init block and the label).
pub fn bossb_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossb_strat);
    let coll = sid(g, bossb_hitflash); // collstrat = hitflashBOSSd (Ouch flash)
    let exp = sid(g, bossb_escape_init); // expstrat = bossBescape (death = flee)
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

/// bossB_strat (GB3STRAT.ASM:1270-1309): face the player; hold at 2500 z, then
/// s_speedto 0 -> bossBdodge_init once stopped. bossB_cont's cosmetic image
/// trail (make_obj boss_B_1 duplicates -> bossBent/bossBspinend) is SCOPED OUT.
fn bossb_strat(g: &mut Game, idx: u16) {
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 4); // s_obj2obj_3dangle roty,rotx,4
    }
    if zdist_more(g, idx, 2500) {
        // .nstop -> bossB_cont3: keep flying forward.
        move_3d(&mut g.objs.aliens[idx as usize]);
    } else {
        // s_speedto x,#0,1,bossBdodge_init — decelerate; switch when stopped.
        let stopped = speed_to(&mut g.objs.aliens[idx as usize], 0, 1);
        if stopped {
            bossbdodge_init(g, idx);
            return;
        }
        move_3d(&mut g.objs.aliens[idx as usize]);
    }
    bossb_cont2(g, idx);
}

/// bossB_cont2/cont4 tail (GB3STRAT.ASM:1302-1308): accumulate the boss bar,
/// stay relative to the player, add the player's Z scroll.
fn bossb_cont2(g: &mut Game, idx: u16) {
    add_bosshp(g, idx);
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

/// bossBdodge_init (GB3STRAT.ASM:1311-1316): enter the dodge attack.
fn bossbdodge_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbdodge_strat);
    let coll = sid(g, bossb_hitflash); // bossBdodgecol -> hitflashBOSSd
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte3 = 1; // s_set_alvar al_sbyte3,#1
        al.collstratptr = Some(coll);
        al.sflags &= !ASF_NOHITAFFECT; // s_clr_alsflag nohitaffect (now damageable)
    }
    bossbdodge_strat(g, idx);
}

/// bossBdodge_strat (GB3STRAT.ASM:1316-1401): the face's main attack. Picks a
/// target quadrant from the player's screen position, chases the move-table
/// point, and fires (laser when facing / homing missile when repositioning).
/// At hp<16 it hands off to the spin wind-up.
fn bossbdodge_strat(g: &mut Game, idx: u16) {
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
            if (ea_random(g) as u8) >= 127 {
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
    let range = range_xy(me.worldx, me.worldy, tx, ty);
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
        if notdelay(g, 3) {
            if let Some(p) = player(g) {
                let m = g.objs.aliens[idx as usize];
                let yaw = angle_xz(&m, &p);
                let pitch = strat_pitch_toward(&m, &p);
                fire_home(g, idx, pitch, yaw); // HMISSILE1 (homing approx)
            }
        }
    }
    bossb_cont2(g, idx);
}

/// bossb_hitflash — collstrat for the face (bossBdodgecol/hitflashBOSSd,
/// GB3STRAT.ASM:1404-1409): reset the retarget timer then flash.
fn bossb_hitflash(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sbyte3 = 1;
    strat_hit_flash(g, idx);
}

/// bossBspin1_init/strat (GB3STRAT.ASM:1436-1459): recenter + rise, spin the
/// pitch, then drop into spin2 once centered. Condensed: cosmetic image flag
/// juggling omitted.
fn bossbspin1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspin1_strat);
    let coll = sid(g, strat_hit_flash);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags2 |= ASF2_SFLAG1;
    al.collstratptr = Some(coll);
    bossbspin1_strat(g, idx);
}
fn bossbspin1_strat(g: &mut Game, idx: u16) {
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

/// bossBspin2_init/strat (GB3STRAT.ASM:1463-1492): accelerate to 120 while
/// spinning rotx down; counts down sbyte1 then to the spinend terminal.
fn bossbspin2_init(g: &mut Game, idx: u16) {
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
fn bossbspin2_strat(g: &mut Game, idx: u16) {
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

/// bossBspinend2_init/strat (GB3STRAT.ASM:1495-1552): the terminal drain phase.
/// Resets the bar to bossBspinHP and keeps firing/draining until the face's HP
/// hits 0 -> the escape expstrat runs. The multi-entity "newent" juggling and
/// scream branch are SCOPED OUT (cosmetic).
fn bossbspinend2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbspinend2_strat);
    let coll = sid(g, strat_hit_flash); // bossBspinendcol -> hitflashBOSSd
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.hp = BOSSB_SPIN_HP;
        al.stratptr = Some(tick);
        al.collstratptr = Some(coll);
        al.sflags &= !ASF_NOHITAFFECT;
        al.sbyte4 = 0;
        al.sword2 = al.hp as i16;
    }
    set_bossmaxhp(g, BOSSB_SPIN_HP as u16);
    bossbspinend2_strat(g, idx);
}
fn bossbspinend2_strat(g: &mut Game, idx: u16) {
    // Recenter + rise, aim, and fire homing lasers on the notdelay-3 gate.
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, 0, 2);
        al.worldy = achase16(al.worldy, SPACE_VIEWCY + 100, 2);
        achase_angle(&mut al.roty, DEG180, 2);
        achase_angle(&mut al.rotx, 0, 2);
    }
    if notdelay_stag(g, idx, 3) {
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            fire_home(g, idx, pitch, yaw);
        }
    }
    bossb_cont2(g, idx);
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
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

// ############################################################
// bossBrob — the robot (GB3STRAT.ASM:1877-3140).
// ############################################################

/// bossBrob_Istrat (GB3STRAT.ASM:1877-1912): wire data/ptrs, pose facing away,
/// then fall into bossBrob_strat. expstrat is the death->explode (ROM chains
/// bossBrobchg's multi-form transform; ported terminal explodes — module doc).
pub fn bossbrob_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_strat);
    let coll = sid(g, bossbrob_col); // hitflashbossD (Ouch) via collstrat
    let exp = sid(g, bossbrob_transform_init); // death -> walking form -> explode
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

/// bossBrob_strat (GB3STRAT.ASM:1914-1938): sway heading between deg180±deg45
/// every 32 frames while approaching; when close (|dz|<=3000 and |worldx|<=200)
/// -> bossBrob2_init. bossB_cont3 = move forward + the shared cont tail.
fn bossbrob_strat(g: &mut Game, idx: u16) {
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

/// bossBrob2_init/strat (GB3STRAT.ASM:1943-1954): brake to a stop, level out,
/// then split.
fn bossbrob2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob2_strat);
    g.objs.aliens[idx as usize].stratptr = Some(tick);
    bossbrob2_strat(g, idx);
}
fn bossbrob2_strat(g: &mut Game, idx: u16) {
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

/// bossBrobcent_srou (GB3STRAT.ASM:1994-2007): recenter to (0,-350) and creep
/// forward if the player is far.
fn bossbrobcent_srou(g: &mut Game, idx: u16) {
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

/// bossBrobsplit_init/strat (GB3STRAT.ASM:1958-1972): recenter until within 50
/// range then split into the three shootable parts.
fn bossbrobsplit_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrobsplit_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags2 &= !ASF2_SFLAG3;
    bossbrobsplit_strat(g, idx);
}
fn bossbrobsplit_strat(g: &mut Game, idx: u16) {
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

/// bossBrobsplit2_init (GB3STRAT.ASM:1977-2001): pick a random 0/1/2, spawn the
/// two OTHER face/hand parts (bossBentsplit) at their relpos, keep the chosen
/// one as the body, and switch the body to bossBentsplit2 (the attacking core).
/// Each spawned part carries the boss bar (add_bosshp), so the fight now drains
/// through the parts. The child link uses child slots 1/2/3.
fn bossbrobsplit2_init(g: &mut Game, idx: u16) {
    // s_set_var2rnd svar_byte1,#3 ; reject 3 (0..2).
    let mut pick = ea_random(g) as u8 & 3;
    while pick == 3 {
        pick = ea_random(g) as u8 & 3;
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

/// bossBrobMent_srou (GB3STRAT.ASM:2059-2069): spawn a duplicate part linked to
/// the mother, running bossBentsplit_Istrat.
fn bossbrob_ment(g: &mut Game, mother: u16, child_num: u8) -> Option<u16> {
    let child = make_obj(g, SH_BOSS_B_1)?;
    copy_pos(g, child, mother);
    copy_rots(g, child, mother);
    if !boss_attach_child_to_mother(g, mother, child, child_num) {
        g.objs.free(child);
        return None;
    }
    bossbentsplit_init(g, child);
    Some(child)
}

/// bossBentsplit_Istrat/strat (GB3STRAT.ASM:2044-2057, 2106-2145): a shootable
/// face/hand part. hardHP (indestructible directly — the fight is timed by the
/// body), fires straight lasers on a gate, drifts to its sword rest-pos, and
/// feeds the boss bar. Its collstrat clears `collide` on hit (a spark). The
/// per-frame anim/depth cosmetics are omitted.
fn bossbentsplit_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbentsplit_strat);
    let coll = sid(g, bossbentsplit_col);
    let al = &mut g.objs.aliens[idx as usize];
    al.hp = HARDHP; // s_set_aldata #hardHP,#0
    al.ap = 0;
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.depthoffset = 1;
    al.count = 8; // lifecnt (entry fade)
    al.sbyte1 = 1;
    al.collflags |= COLLTYPE_ENEMY2 | COLLTYPE_ENEMYWEAP;
}
fn bossbentsplit_col(g: &mut Game, idx: u16) {
    // trigse $27 spark; clear the collide bit so it reads as an impact.
    strat_hit_flash(g, idx);
}
fn bossbentsplit_strat(g: &mut Game, idx: u16) {
    // s_jmp_notdelay 4,al1pt -> straight laser volley.
    if notdelay_stag(g, idx, 4) {
        aim_and_fire_laser(g, idx, 0);
    }
    // Drift toward the rest position (sword1/sword2) unless the counter is low.
    let sb1 = {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 != 0 {
            al.sbyte1 -= 1;
        } else {
            al.sbyte1 = 100;
        }
        al.sbyte1
    };
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

/// bossBentsplit2_Istrat/strat (GB3STRAT.ASM:2015-2039): the aggressive central
/// part — aim + fire homing lasers, feed the bar, and loop back to re-split
/// when its counter runs out.
fn bossbentsplit2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbentsplit2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sflags &= !ASF_NOHITAFFECT;
    // bossBentsplit_Icont sets sbyte1=1 then bossBentsplit_cont's decbne
    // immediately underflows it (1->0) and resets to 100 (GB3STRAT.ASM:2126).
    al.sbyte1 = 100;
    bossbentsplit2_strat(g, idx);
}
fn bossbentsplit2_strat(g: &mut Game, idx: u16) {
    if let Some(p) = player(g) {
        strat_aim_3d(g, idx, &p, 2);
        if notdelay_stag(g, idx, 3) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            fire_home(g, idx, pitch, yaw); // RELSLOWELASERHOME
        }
    }
    add_bosshp(g, idx);
    // s_jmp_alvarNE al_sbyte1,#1,cont ; re-split only when the timer hits 1,
    // else bossBentsplit_cont decrements it (100->...->1, GB3STRAT.ASM:2039).
    if g.objs.aliens[idx as usize].sbyte1 == 1 {
        bossbrobsplit2_init(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 != 0 {
            al.sbyte1 -= 1;
        } else {
            al.sbyte1 = 100;
        }
    }
    boss_keeprel_to_player(g, idx);
    add_player_z(g, idx);
}

/// bossBrob_col — the robot's collstrat (bossBrobcol_Istrat, GB3STRAT.ASM:
/// 2947-2975 + bossBrobsepcol:2810-2833): route a hit to the directional Ouch
/// reaction by hitflags zone (top/left/right), else a normal flash.
fn bossbrob_col(g: &mut Game, idx: u16) {
    let hf = g.objs.aliens[idx as usize].hitflags;
    let sbyte4 = if hf & HF1 != 0 {
        Some(2 * 32) // top
    } else if hf & HF2 != 0 {
        Some(0) // left
    } else if hf & HF3 != 0 {
        Some(32) // right
    } else {
        None
    };
    match sbyte4 {
        Some(v) => {
            let al = &mut g.objs.aliens[idx as usize];
            al.sbyte4 = v;
            al.hitflags = 0;
            al.sbyte3 = 16; // Ouch duration
            al.sflags2 |= ASF2_SFLAG1; // sflag5 "ouching" (bit reused)
        }
        None => {
            g.objs.aliens[idx as usize].hitflags = 0;
            strat_hit_flash(g, idx);
        }
    }
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
        1 | 5 => bossbrob_firep1_init(g, idx),
        2 => bossbrob_pouncepos_init(g, idx),
        3 | 6 | 8 => bossbrob_farjump1_init(g, idx),
        4 => bossbrob_sep_init(g, idx),
        7 => bossbrob_kick_init(g, idx),
        _ => bossbrob_kick_init(g, idx),
    }
}

/// bossbrobfireP1 (GB3STRAT.ASM:2409-2426): plant in front of the player and
/// fire HPLASMA on a gate for ~60 ticks, then bossBrobstart2 (-> reset the
/// cycle). Ported: run the Ouch reaction, fire, then next state at count 0.
fn bossbrob_firep1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_firep1_strat);
    let coll = sid(g, bossbrob_col);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(coll);
    al.sbyte1 = 60;
    bossbrob_firep1_strat(g, idx);
}
fn bossbrob_firep1_strat(g: &mut Game, idx: u16) {
    bossbrob_frontplayer(g, idx, 1400);
    // s_beqdec_alvar al_sbyte1,bossBrobstart2_init
    if g.objs.aliens[idx as usize].sbyte1 == 0 {
        bossbrob_nextstate(g, idx);
        return;
    }
    g.objs.aliens[idx as usize].sbyte1 -= 1;
    if notdelay(g, 4) {
        // HPLASMA straight shot forward.
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let _ = spawn_projectile(g, Some(idx), 0, 0, 0, m.rotx, yaw, 60, 50, 10, ACF_COLLTYPE4);
        }
    }
    bossbrob_ouch(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// bossBrobfrontplayer_srou (GB3STRAT.ASM:2434-2453): recenter to (0,-80<<scale)
/// and aim yaw at the player, closing to `front` z ahead of the player.
fn bossbrob_frontplayer(g: &mut Game, idx: u16, front: i16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = achase16(al.worldx, 0, 3);
        al.worldy = achase16(al.worldy, -80 << BOSSB_SCALE, 3);
    }
    if let Some(p) = player(g) {
        let m = g.objs.aliens[idx as usize];
        let yaw = angle_xz(&m, &p);
        achase_angle(&mut g.objs.aliens[idx as usize].roty, yaw, 3);
        let target_z = p.worldz.wrapping_add(front);
        let al = &mut g.objs.aliens[idx as usize];
        al.worldz = achase16(al.worldz, target_z, 3);
    }
}

/// bossBrobfarjump1/2/land (GB3STRAT.ASM:2739-2810): a leap toward the player.
/// Ported condensed to a single jump-arc: launch up, gravity down, land, then
/// next state. Per-frame anim frames scoped out.
fn bossbrob_farjump1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_farjump_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sflags |= ASF_NOHITAFFECT;
        al.vel = 100; // s_set_speed #100 (jump velocity)
        al.vy = -100; // s_set_alvar al_vy,#-100
        al.stratstate = 0; // reuse as an internal launched/landing sub-flag
    }
    // s_gen_vecs (vx/vz from roty) — vy kept.
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    bossbrob_farjump_strat(g, idx);
}
fn bossbrob_farjump_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let ground = (-80i16) << BOSSB_SCALE;
    let landed = g.objs.aliens[idx as usize].worldy >= ground;
    if landed {
        // bossbrobfarland_end: clear nohitaffect, next state.
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.worldy = ground;
            al.vy = 0;
            al.sflags &= !ASF_NOHITAFFECT;
        }
        bossbrob_nextstate(g, idx);
        return;
    }
    let px = player(g).map(|p| p.worldx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vy = al.vy.wrapping_add(8); // gravity
        achase_angle(&mut al.roty, DEG180, 3);
        if let Some(px) = px {
            al.worldx = achase16(al.worldx, px, 2);
        }
    }
    bossbrob_ouch(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// bossBrobPouncepos/Pounce2 (GB3STRAT.ASM:2453-2540): crouch, then pounce down
/// at the player. Ported condensed: dive toward the player then next state.
fn bossbrob_pouncepos_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_pounce_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 30;
        al.vy = -75;
        al.rotx = 8;
    }
    gen_vecs_3d(&mut g.objs.aliens[idx as usize]);
    bossbrob_pounce_strat(g, idx);
}
fn bossbrob_pounce_strat(g: &mut Game, idx: u16) {
    apply_velocity(&mut g.objs.aliens[idx as usize]);
    let ground = (-80i16) << BOSSB_SCALE;
    if g.objs.aliens[idx as usize].worldy >= ground {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = ground;
        al.vx = 0;
        al.vy = 0;
        al.vz = 0;
        bossbrob_nextstate(g, idx);
        return;
    }
    let al = &mut g.objs.aliens[idx as usize];
    al.vy = al.vy.wrapping_add(4);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// bossBrobsep (GB3STRAT.ASM:2760-2810): spin up, spawn a couple of drifting
/// parts, then to the random-position strafing phase. Ported condensed: spawn
/// two parts once, then next state (the sep/pounce hit-count mechanic is
/// scoped; the parts still feed the bar and fire).
fn bossbrob_sep_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_sep_strat);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.stratptr = Some(tick);
        al.sbyte1 = 2;
        al.sbyte2 = 2;
        al.sflags2 &= !ASF2_SFLAG1;
        al.sflags |= ASF_NOHITAFFECT;
    }
    // Spawn two drifting parts (child slots 1/2).
    for cn in 1..=2u8 {
        if let Some(child) = bossbrob_ment(g, idx, cn) {
            g.objs.aliens[child as usize].sflags |= ASF_COLLDISABLE;
        }
    }
    bossbrob_sep_strat(g, idx);
}
fn bossbrob_sep_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        if al.roty != DEG180 {
            al.roty = al.roty.wrapping_add(4);
        }
    }
    // Count down then next state (condensed; ROM gates on Z distance).
    let done = {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 != 0 {
            al.sbyte1 -= 1;
        }
        al.sbyte1 == 0
    };
    if done {
        let al = &mut g.objs.aliens[idx as usize];
        al.sflags &= !ASF_NOHITAFFECT;
        bossbrob_nextstate(g, idx);
        return;
    }
    move_3d(&mut g.objs.aliens[idx as usize]);
    add_bosshp(g, idx);
    add_player_z(g, idx);
}

/// bossBrobkick (GB3STRAT.ASM:2695-2726): a kicking strike that (in ROM) spawns
/// a homing foot. Ported: strike toward the player, fire the "foot" as a homing
/// shot at anim-mid, count down, next state. The kick anim frames are scoped.
fn bossbrob_kick_init(g: &mut Game, idx: u16) {
    let tick = sid(g, bossbrob_kick_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.sbyte1 = 20;
    al.animframe = 0;
    bossbrob_kick_strat(g, idx);
}
fn bossbrob_kick_strat(g: &mut Game, idx: u16) {
    // Fire the "foot" once, mid-kick (at counter 10).
    if g.objs.aliens[idx as usize].sbyte1 == 10 {
        if let Some(p) = player(g) {
            let m = g.objs.aliens[idx as usize];
            let yaw = angle_xz(&m, &p);
            let pitch = strat_pitch_toward(&m, &p);
            fire_home(g, idx, pitch, yaw);
        }
    }
    let done = {
        let al = &mut g.objs.aliens[idx as usize];
        if al.sbyte1 != 0 {
            al.sbyte1 -= 1;
        }
        al.sbyte1 == 0
    };
    if done {
        bossbrob_nextstate(g, idx);
        return;
    }
    bossbrob_ouch(g, idx);
    add_bosshp(g, idx);
    add_player_z(g, idx);
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

/// bossBrobOuch_srou (GB3STRAT.ASM:3068-3095): while sbyte3>0 (in an Ouch),
/// apply the current (rotx,roty) knockback frame, step the frame, and lob a
/// BOSSHMISSILE1 counter-shot on a gate. sbyte4 = zone byte-offset (0/32/64) +
/// 2 per frame. When the count expires, ease rotx back to 0.
fn bossbrob_ouch(g: &mut Game, idx: u16) {
    // sflag5 "ouching" reused ASF2_SFLAG1 in bossbrob_col; gate on sbyte3>0.
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
        al.sflags2 &= !ASF2_SFLAG1;
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
            fire_home(g, idx, pitch, yaw); // BOSSHMISSILE1 counter-shot
        }
    }
}

// ---- death ------------------------------------------------------------------

/// sflags3 latch: this robot has already transformed to the walking form.
const BOSSBROB_WALKING: u8 = 0x01;
const BOSSBROB_WALK_HP: u8 = 60; // bossBrobchg4 caps al_hp at 60 (GB3STRAT.ASM:2246)

/// bossBrob's expstrat — a TWO-PHASE bridge that condenses ROM's death path.
///
/// Phase 1 (floating form dies): ROM chains bossBrobchg -> chg2/3/4 ->
/// bossBrobstart -> bossBrobstart2 -> the bossBrobnextstate attack cycle
/// (GB3STRAT.ASM:2153-2318) — the robot morphs to the walking-legs Andross with
/// a fresh HP pool and begins jumping/kicking/firing. The multi-frame morph
/// animation + music is SCOPED OUT (module doc); we reset the bar and enter the
/// attack cycle directly.
///
/// Phase 2 (walking form dies): the real death — bossBrobsepexp/bossBrobexp ->
/// bossBpexp2 (GB3STRAT.ASM:2860-2941) sets GF_BOSSDEAD and stages the boss
/// explosion. Ported via the shared strat_boss_explode_init.
pub fn bossbrob_transform_init(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].sflags3 & BOSSBROB_WALKING == 0 {
        // Phase 1: become the walking form and start the attack rotation.
        let coll = sid(g, bossbrob_col);
        {
            let al = &mut g.objs.aliens[idx as usize];
            al.sflags3 |= BOSSBROB_WALKING;
            al.hp = BOSSBROB_WALK_HP;
            al.ap = BOSSB_AP;
            al.collstratptr = Some(coll);
            al.sflags &= !ASF_NOHITAFFECT;
            al.roty = DEG180;
            al.rotx = 0;
            al.vy = 0;
            al.stratstate = 0; // s_next_state -> 1 on entry
        }
        set_bossmaxhp(g, BOSSBROB_WALK_HP as u16);
        bossbrob_nextstate(g, idx); // -> fireP1 (state 1)
        return;
    }
    // Phase 2: the true death.
    g.vars.gameflags |= GF_BOSSDEAD;
    strat_boss_explode_init(g, idx);
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
    // bossBrob (IS 118) is resolvable by flat/synthetic id once MAP1_6A is
    // ported (currently stubbed in sf-map route1/level1_6.rs).
}
