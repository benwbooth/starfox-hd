//! Mother-object system: the mothermap bytecode interpreter (`bemother_l`,
//! ASM/MOTHER.ASM) plus the strategies that drive it and the asteroid-wave
//! child strategies referenced by MOTHERS.ASM data.
//!
//! ROM model:
//! - the map VM `mapmother` opcode (WORLD.ASM:1894-1955) spawns a mother
//!   object whose `al_ptr` points at a mothermap (data: sf-map `mothers`),
//!   and whose strategy is `mother1_istrat`/`mother2_istrat`
//!   (D2STRATS.ASM:501-541);
//! - every tick the mother strategy re-pins its Z to the player
//!   (`worldz = player.z + sword2`) and calls `bemother_l`;
//! - `bemother_l` (MOTHER.ASM:36-77) runs mothermap entries whenever
//!   `al_sword1 <= 0`, otherwise decrements it by `lastzchange` — waits are
//!   world-Z distances, not frames. Executing an entry loads its
//!   `moth_count` into `al_sword1` (the wait AFTER the entry) and
//!   dispatches on `moth_ctrl`:
//!     0 motherobj   spawn child at mother pos + (x,y,z)      (MOTHER.ASM:81)
//!     2 motherloop  al_sbyte3 counter, jump while < count    (:262)
//!     4 motherend   remove the mother, stop                  (:294)
//!     6 motherrnd   spawn child at mother pos + random offset
//!                   in [-mask/2, mask/2) per axis; consumes 6
//!                   RNG bytes (x lo/hi, y lo/hi, z lo/hi)     (:306)
//!     8 mothergoto  al_ptr = target                           (:174)
//!    10 motherwait  advance (the count field is the wait)      (:165)
//!    12 mothercnt   motheraccum = live objects with shape      (:236)
//!    14 motherjump  conditional jump on motheraccum vs value   (:185)
//!   Children spawn with `al_type = atzremove`, no coll/exp strats, and
//!   their strategy from the entry's 24-bit address.
//!
//! Known deviations (documented, low-risk):
//! - ROM `mother1/2_istrat` temporarily repoint `allst` at the mother (or
//!   player) so children are list-inserted next to it and run their istrat
//!   the same frame; the Rust object pool pushes new objects at the list
//!   head, so children run their init one frame later and `mothercnt`
//!   scans the whole list (no map shipped in MOTHERS.ASM relies on the
//!   partial scan).
//! - `s_sprite_obj` (2D sprite promotion) and the display-only
//!   `drotsflat_x`/`dobj2obj3dangle_xy` calls in the meteor inits are not
//!   ported (renderer lane); motion, HP/AP, collision and RNG order match
//!   the ROM.

use crate::common::{
    sf_random, strat_apply_velocity, strat_gen_vecs_3d, strat_init_obj_vars, strat_remove_obj,
};
use crate::enemy_a::{sid, strat_explode, strat_hit_flash, COLLTYPE_ENEMY1};
use sf_game::alien::{StratId, ASF_COLLDISABLE, ASF_NOHITAFFECT, ATZREMOVE};
use sf_game::game::Game;
use sf_map::consts::{
    STRAT_ADDR_CLASTEROID, STRAT_ADDR_METEOR, STRAT_ADDR_MOTHER1, STRAT_ADDR_MOTHER2,
    STRAT_ADDR_SEARCHMETEOR, STRAT_ADDR_SLOWMETEOR,
};
use sf_map::mothers::{mop, mother_maps, MO_SIZEOF, MOTH_SIZEOF};

// STRATEQU.INC:212-213.
const METEOR_HP: u8 = 2;
const METEOR_AP: u8 = 12;

/// `motheraccum` (WRAM $17DA) — mothercnt/motherjump accumulator. Lives in
/// the GameVars WRAM mirror right after the strat-lane `sv` block
/// (common.rs uses 0x0500..0x0570).
const MOTHERACCUM: u16 = 0x0570;

/// Register the mother-system strategies at their synthetic addresses
/// (sf-map consts STRAT_ADDR_MOTHER1/2 + the mothermap child strats).
/// Called from `table::register_all`.
pub fn register(g: &mut Game) {
    let m1 = sid(g, strat_mother1_init);
    let m2 = sid(g, strat_mother2_init);
    let meteor = sid(g, strat_meteor_init);
    let slow = sid(g, strat_slowmeteor_init);
    let search = sid(g, strat_searchmeteor_init);
    let clast = sid(g, strat_clasteroid_init);
    g.world.register_strategy_address(STRAT_ADDR_MOTHER1, m1);
    g.world.register_strategy_address(STRAT_ADDR_MOTHER2, m2);
    g.world.register_strategy_address(STRAT_ADDR_METEOR, meteor);
    g.world.register_strategy_address(STRAT_ADDR_SLOWMETEOR, slow);
    g.world
        .register_strategy_address(STRAT_ADDR_SEARCHMETEOR, search);
    g.world.register_strategy_address(STRAT_ADDR_CLASTEROID, clast);
}

// ============================================================
// Mother strategies (D2STRATS.ASM:501-541)
// ============================================================

/// Live player Z (ROM `s_set_objtobeplayer` reads `playpt`; mirror the
/// `init_strats_l` playpt-then-slot-0 fallback).
fn player_z(g: &Game) -> i16 {
    let playpt = g.vars.internal_playpt;
    if playpt >= 0 && (playpt as usize) < sf_game::alien::NUMBER_AL {
        let al = &g.objs.aliens[playpt as usize];
        if al.active {
            return al.worldz;
        }
    }
    if g.objs.aliens[0].active {
        return g.objs.aliens[0].worldz;
    }
    0
}

/// `mother1_istrat` (D2STRATS.ASM:501): latch the Z offset from the player
/// into `al_sword2`, then fall through into the tick.
pub fn strat_mother1_init(g: &mut Game, idx: u16) {
    let tick = sid(g, strat_mother1_tick);
    mother_init_common(g, idx, tick);
    strat_mother1_tick(g, idx);
}

/// `mother2_istrat` (D2STRATS.ASM:524). Identical to mother1 except the
/// ROM's `allst` repoint (player instead of self) — see module doc.
pub fn strat_mother2_init(g: &mut Game, idx: u16) {
    let tick = sid(g, strat_mother2_tick);
    mother_init_common(g, idx, tick);
    strat_mother2_tick(g, idx);
}

fn mother_init_common(g: &mut Game, idx: u16, tick: StratId) {
    let pz = player_z(g);
    let al = &mut g.objs.aliens[idx as usize];
    // s_copy_alvar2alvar W,x,al_sword2,x,al_worldz / s_sub_alvars ...,y,al_worldz
    al.sword2 = al.worldz.wrapping_sub(pz);
    al.stratptr = Some(tick);
}

fn mother_tick_common(g: &mut Game, idx: u16) {
    let pz = player_z(g);
    let al = &mut g.objs.aliens[idx as usize];
    // s_copy W,x,al_worldz,y,al_worldz / s_add_alvars W,x,al_worldz,x,al_sword2
    al.worldz = pz.wrapping_add(al.sword2);
    bemother(g, idx);
}

fn strat_mother1_tick(g: &mut Game, idx: u16) {
    mother_tick_common(g, idx);
}

fn strat_mother2_tick(g: &mut Game, idx: u16) {
    mother_tick_common(g, idx);
}

// ============================================================
// bemother — the mothermap interpreter (MOTHER.ASM:36-77)
// ============================================================

/// Safety bound only (the ROM has none): a well-formed map always reaches
/// a positive wait; this stops a malformed all-zero-count cycle.
const MAX_OPS_PER_TICK: u32 = 64;

/// `bemother_l` against the global MOTHERS.ASM blob.
pub fn bemother(g: &mut Game, idx: u16) {
    bemother_on(g, idx, &mother_maps().blob);
}

fn rd16(mm: &[u8], p: usize) -> u16 {
    if p + 1 < mm.len() {
        mm[p] as u16 | ((mm[p + 1] as u16) << 8)
    } else {
        0
    }
}

/// Interpreter core, parameterized over the blob for tests.
pub fn bemother_on(g: &mut Game, idx: u16, mm: &[u8]) {
    for _ in 0..MAX_OPS_PER_TICK {
        // .nxtone: lda al_sword1,x / bmi .doobj / bne .out
        let sw = g.objs.aliens[idx as usize].sword1;
        if sw > 0 {
            // .out: sec / sbc lastzchange / sta al_sword1,x
            let lzc = g.world.lastzchange;
            g.objs.aliens[idx as usize].sword1 = sw.wrapping_sub(lzc);
            return;
        }
        let p = g.objs.aliens[idx as usize].ptr as usize;
        if p == 0 || p >= mm.len() {
            return; // no mothermap attached (al_ptr 0 = pad byte)
        }
        // lda.l moth_count,x -> sta al_sword1,y (the wait AFTER this entry).
        // The ROM does this before dispatch for every ctrl, including
        // motherend (whose entry is 1 byte — the read is junk but the
        // mother is removed anyway).
        g.objs.aliens[idx as usize].sword1 = rd16(mm, p + 1) as i16;
        match mm[p] {
            mop::OBJ => mother_spawn(g, idx, mm, p, false),
            mop::RND => mother_spawn(g, idx, mm, p, true),
            mop::LOOP => {
                // motherloop: ml_count at +5, ml_loop at +3; al_sbyte3 is
                // the iteration counter (cmp/bmi = 8-bit signed compare).
                let count = if p + 5 < mm.len() { mm[p + 5] } else { 0 };
                let target = rd16(mm, p + 3);
                let al = &mut g.objs.aliens[idx as usize];
                let c = al.sbyte3.wrapping_add(1);
                if (c.wrapping_sub(count) as i8) < 0 {
                    al.sbyte3 = c;
                    al.ptr = target;
                } else {
                    al.sbyte3 = 0;
                    al.ptr = (p + sf_map::mothers::ML_SIZEOF) as u16;
                }
            }
            mop::END => {
                // motherend: s_remove_obj + abort.
                strat_remove_obj(g);
                return;
            }
            mop::GOTO => {
                g.objs.aliens[idx as usize].ptr = rd16(mm, p + 3);
            }
            mop::WAIT => {
                g.objs.aliens[idx as usize].ptr = (p + MOTH_SIZEOF) as u16;
            }
            mop::COUNT => {
                // mothercnt: motheraccum = live objects with mc_shape.
                let shape = rd16(mm, p + 3);
                let mut accum: u16 = 0;
                for i in g.objs.active_indices() {
                    if g.objs.aliens[i as usize].shape == shape {
                        accum = accum.wrapping_add(1);
                    }
                }
                g.vars.write_ext16(MOTHERACCUM, accum);
                g.objs.aliens[idx as usize].ptr = (p + sf_map::mothers::MC_SIZEOF) as u16;
            }
            mop::JUMP => {
                // motherjump: mj_val +3, mj_addr +5, mj_func +7. Branch
                // senses are the ROM's (MOTHER.ASM:190-232): GT jumps when
                // accum < val (bcc), LT when accum >= val (bcs).
                let val = rd16(mm, p + 3);
                let addr = rd16(mm, p + 5);
                let func = if p + 7 < mm.len() { mm[p + 7] } else { 0xFF };
                let accum = g.vars.read_ext16(MOTHERACCUM);
                let jump = match func {
                    0 => accum == val,          // mj_EQ
                    1 => accum != val,          // mj_NE
                    2 => accum < val,           // mj_GT (bcc .jump)
                    3 => accum >= val,          // mj_LT (bcs .jump)
                    _ => false,                 // .nojump fallthrough
                };
                let al = &mut g.objs.aliens[idx as usize];
                al.ptr = if jump {
                    addr
                } else {
                    (p + sf_map::mothers::MJ_SIZEOF) as u16
                };
            }
            _ => return, // motherset (16) has no ROM jump-table entry
        }
    }
    debug_assert!(false, "bemother: runaway mothermap (all-zero waits?)");
}

/// motherobj / motherrnd child spawn (MOTHER.ASM:81-160 / 306-435).
fn mother_spawn(g: &mut Game, idx: u16, mm: &[u8], p: usize, random: bool) {
    let mx = g.objs.aliens[idx as usize].worldx;
    let my = g.objs.aliens[idx as usize].worldy;
    let mz = g.objs.aliens[idx as usize].worldz;

    let (ox, oy, oz);
    if random {
        // motherrnd always consumes 6 RNG bytes (x lo/hi, y lo/hi, z lo/hi)
        // BEFORE masking, even for zero masks (MOTHER.ASM:313-328).
        let rx = rand16(g);
        let ry = rand16(g);
        let rz = rand16(g);
        ox = mask_off(rd16(mm, p + 3), rx);
        oy = mask_off(rd16(mm, p + 5), ry);
        oz = mask_off(rd16(mm, p + 7), rz);
    } else {
        ox = rd16(mm, p + 3) as i16;
        oy = rd16(mm, p + 5) as i16;
        oz = rd16(mm, p + 7) as i16;
    }
    let shape = rd16(mm, p + 9);
    let strat24 = rd16(mm, p + 11) as u32 | ((*mm.get(p + 13).unwrap_or(&0) as u32) << 16);

    // l_add allst,alfreelst,.nofreeblks — on no free block, just skip the
    // entry (MOTHER.ASM:153-160).
    if let Some(ci) = g.objs.alloc() {
        strat_init_obj_vars(&mut g.objs.aliens[ci as usize]);
        let strat = g.world.find_strategy_address(strat24);
        let al = &mut g.objs.aliens[ci as usize];
        al.worldx = mx.wrapping_add(ox);
        al.worldy = my.wrapping_add(oy);
        al.worldz = mz.wrapping_add(oz);
        al.shape = shape;
        al.stratptr = strat; // None => inert child (unported strat lane)
        al.collstratptr = None; // stz alx_collstratptr
        al.expstratptr = None; // stz alx_expstratptr
        al.flags = 0;
        al.sflags = 0;
        al.rotx = 0;
        al.roty = 0;
        al.rotz = 0;
        al.type_ = ATZREMOVE;
    }
    g.objs.aliens[idx as usize].ptr = (p + MO_SIZEOF) as u16;
}

/// Two `random_l` calls assembled low-then-high (MOTHER.ASM:314-317).
fn rand16(g: &mut Game) -> u16 {
    let lo = sf_random(&mut g.vars) & 0xFF;
    let hi = sf_random(&mut g.vars) & 0xFF;
    lo | (hi << 8)
}

/// motherrnd mask: offset = (rand & (mask-1)) - mask/2, 16-bit wrapping
/// (MOTHER.ASM:332-340). Zero mask = zero offset.
fn mask_off(mask: u16, r: u16) -> i16 {
    if mask == 0 {
        0
    } else {
        (r & mask.wrapping_sub(1)).wrapping_sub(mask >> 1) as i16
    }
}

// ============================================================
// Child strategies: meteor family (DSTRATS.ASM:1167-1246) and
// clasteroid (GA2STRAT.ASM:3357-3366)
// ============================================================

/// `meteor_istrat` (DSTRATS.ASM:1215): sword1 = 60 fall speed.
pub fn strat_meteor_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sword1 = 60;
    meteor_init_common(g, idx);
}

/// `slowmeteor_istrat` (DSTRATS.ASM:1199): sword1 = 20, then meteor `.in`.
pub fn strat_slowmeteor_init(g: &mut Game, idx: u16) {
    g.objs.aliens[idx as usize].sword1 = 20;
    meteor_init_common(g, idx);
}

/// meteor_istrat `.in`..`meteor_istrat3` shared body. RNG order: vel(&7),
/// sbyte1(&3), roty, then the 50% `s_jmp_random` byte.
fn meteor_init_common(g: &mut Game, idx: u16) {
    let hit = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let tick = sid(g, strat_meteor_tick);
    let r_vel = (sf_random(&mut g.vars) & 7) as u8;
    let r_sb1 = (sf_random(&mut g.vars) & 3) as u8;
    let r_roty = (sf_random(&mut g.vars) & 0xFF) as u8;
    let r_clear = (sf_random(&mut g.vars) & 0xFF) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(hit);
    al.expstratptr = Some(exp);
    al.hp = METEOR_HP;
    al.ap = METEOR_AP;
    al.rotz = 0;
    al.vel = r_vel;
    al.sbyte1 = r_sb1;
    al.roty = r_roty;
    // s_cmp_alvar B,x,al_roty,#128 / s_bcc .oneway / s_neg_alvar sbyte1
    if r_roty >= 128 {
        al.sbyte1 = al.sbyte1.wrapping_neg();
    }
    // s_jmp_random .noclear (50%: rnd < 127 skips the clear)
    if r_clear >= 127 {
        al.sbyte1 = 0;
    }
    strat_gen_vecs_3d(al); // s_jsr dgen3dvecs (rotx=0 pitch)
    al.collflags |= COLLTYPE_ENEMY1; // s_set_colltype x,ENEMY1
    al.sflags |= ASF_NOHITAFFECT; // s_set_alsflag x,nohitaffect
    // ROM falls through meteor_istrat3 into meteor_strat the same frame.
    strat_meteor_tick(g, idx);
}

/// `meteor_strat` (DSTRATS.ASM:1238): drift toward the player Z, tumble,
/// apply velocity. (The ROM skips the Z-pull for asteroid3 fragments;
/// asteroid3 / meteorexp splitting is not ported.)
fn strat_meteor_tick(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_sub(al.sword1); // s_sub_alvars worldz,sword1
    al.rotz = al.rotz.wrapping_add(al.sbyte1); // s_add_alvars rotz,sbyte1
    strat_apply_velocity(al); // s_jsr daddvecs2pos_x
}

/// `searchmeteor_istrat` (DSTRATS.ASM:1167): same random setup, then the
/// tick homes X/Y toward the player (achase rate 4).
pub fn strat_searchmeteor_init(g: &mut Game, idx: u16) {
    let hit = sid(g, strat_hit_flash);
    let exp = sid(g, strat_explode);
    let tick = sid(g, strat_searchmeteor_tick);
    let r_vel = (sf_random(&mut g.vars) & 7) as u8;
    let r_sb1 = (sf_random(&mut g.vars) & 3) as u8;
    let r_roty = (sf_random(&mut g.vars) & 0xFF) as u8;
    let r_clear = (sf_random(&mut g.vars) & 0xFF) as u8;
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(tick);
    al.collstratptr = Some(hit);
    al.expstratptr = Some(exp);
    al.hp = METEOR_HP;
    al.ap = METEOR_AP;
    al.vel = r_vel;
    al.sbyte1 = r_sb1;
    al.roty = r_roty;
    if r_roty >= 128 {
        al.sbyte1 = al.sbyte1.wrapping_neg();
    }
    if r_clear >= 127 {
        al.sbyte1 = 0;
    }
    strat_gen_vecs_3d(al); // s_jsr dgen3dvecs
    al.collflags |= COLLTYPE_ENEMY1;
    al.sflags |= ASF_NOHITAFFECT;
    strat_searchmeteor_tick(g, idx);
}

/// STRATROU `Achase_var2A` word form: step toward target by diff>>rate,
/// with the small-diff clamp so it always makes progress.
fn achase16(cur: i16, target: i16, rate: u32) -> i16 {
    let mut d = target as i32 - cur as i32;
    if d == 0 {
        return cur;
    }
    let min = 1i32 << rate;
    if d > -min && d < min {
        d = if d < 0 { -min } else { min };
    }
    cur.wrapping_add((d >> rate) as i16)
}

fn strat_searchmeteor_tick(g: &mut Game, idx: u16) {
    let (px, py) = {
        let playpt = g.vars.internal_playpt;
        let pi = if playpt >= 0
            && (playpt as usize) < sf_game::alien::NUMBER_AL
            && g.objs.aliens[playpt as usize].active
        {
            playpt as usize
        } else {
            0
        };
        let p = &g.objs.aliens[pi];
        (p.worldx, p.worldy)
    };
    let al = &mut g.objs.aliens[idx as usize];
    // s_achase_alvar2alvar.w W,x,al_worldx,y,al_worldx,4 (and Y).
    al.worldx = achase16(al.worldx, px, 4);
    al.worldy = achase16(al.worldy, py, 4);
    al.rotz = al.rotz.wrapping_add(al.sbyte1);
    strat_apply_velocity(al);
}

/// `clasteroid_Istrat` (GA2STRAT.ASM:3357): collision-disabled drifting
/// rock for the clear demos; lifecnt 70 then self-remove.
pub fn strat_clasteroid_init(g: &mut Game, idx: u16) {
    let tick = sid(g, strat_clasteroid_tick);
    let al = &mut g.objs.aliens[idx as usize];
    al.sflags |= ASF_COLLDISABLE; // s_set_alsflag x,colldisable
    al.stratptr = Some(tick);
    al.count = 70; // s_set_lifecnt x,#70
    strat_clasteroid_tick(g, idx);
}

fn strat_clasteroid_tick(g: &mut Game, idx: u16) {
    // s_dec_lifecnt x: dec al_count, remove at zero.
    let al = &mut g.objs.aliens[idx as usize];
    al.count = al.count.wrapping_sub(1);
    if al.count == 0 {
        strat_remove_obj(g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_map::mothers::mop;

    /// Tiny mothermap: pad byte, then
    ///   motherobj count=100 off=(10,20,30) shape=42 strat=synth hard (226)
    ///   mothergoto 0 -> back to the obj
    fn test_blob() -> (Vec<u8>, u16) {
        let mut v = vec![0xFF];
        let entry = v.len() as u16;
        v.push(mop::OBJ);
        v.extend_from_slice(&100u16.to_le_bytes());
        v.extend_from_slice(&10i16.to_le_bytes());
        v.extend_from_slice(&20i16.to_le_bytes());
        v.extend_from_slice(&30i16.to_le_bytes());
        v.extend_from_slice(&42u16.to_le_bytes());
        let strat24: u32 = 0x020000 | 226;
        v.extend_from_slice(&((strat24 & 0xFFFF) as u16).to_le_bytes());
        v.push((strat24 >> 16) as u8);
        v.push(mop::GOTO);
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&entry.to_le_bytes());
        (v, entry)
    }

    fn game_with_mother(ptr: u16) -> (Game, u16) {
        let mut g = Game::new();
        crate::table::register_all(&mut g);
        // Player in slot 0 (alloc grabs the free head deterministically).
        let p = g.objs.alloc().expect("player slot");
        g.objs.aliens[p as usize].worldz = 1000;
        g.vars.internal_playpt = p as i16;
        let m = g.objs.alloc().expect("mother slot");
        {
            let al = &mut g.objs.aliens[m as usize];
            al.worldx = 5;
            al.worldy = -5;
            al.worldz = 4000;
            al.ptr = ptr;
        }
        g.world.lastzchange = 65;
        (g, m)
    }

    fn count_shape(g: &Game, shape: u16) -> usize {
        g.objs
            .active_indices()
            .into_iter()
            .filter(|&i| g.objs.aliens[i as usize].shape == shape)
            .count()
    }

    /// motherobj spawns at mother pos + entry offset with the entry's shape
    /// and strategy; the entry count becomes the Z-distance wait
    /// (sword1 = count - lastzchange on the same tick, MOTHER.ASM:73-75).
    #[test]
    fn motherobj_spawn_and_wait_cadence() {
        let (blob, entry) = test_blob();
        let (mut g, m) = game_with_mother(entry);

        bemother_on(&mut g, m, &blob);
        assert_eq!(count_shape(&g, 42), 1, "one child after first tick");
        let ci = g
            .objs
            .active_indices()
            .into_iter()
            .find(|&i| g.objs.aliens[i as usize].shape == 42)
            .unwrap();
        let c = &g.objs.aliens[ci as usize];
        assert_eq!((c.worldx, c.worldy, c.worldz), (15, 15, 4030));
        assert_eq!(c.type_, ATZREMOVE);
        assert!(c.stratptr.is_some(), "synth istrat 226 (hard) resolves");
        // wait: 100 loaded, then -65 on the same tick.
        assert_eq!(g.objs.aliens[m as usize].sword1, 35);
        // The pointer rests on the goto entry until the wait elapses
        // (ROM re-checks al_sword1 before every entry, MOTHER.ASM:39-42).
        assert_eq!(g.objs.aliens[m as usize].ptr, entry + 14);

        // Tick 2: 35 > 0 -> wait only, no spawn.
        bemother_on(&mut g, m, &blob);
        assert_eq!(count_shape(&g, 42), 1);
        assert_eq!(g.objs.aliens[m as usize].sword1, -30);

        // Tick 3: -30 <= 0 -> next spawn.
        bemother_on(&mut g, m, &blob);
        assert_eq!(count_shape(&g, 42), 2);
        assert_eq!(g.objs.aliens[m as usize].sword1, 35);
    }

    /// motherrnd: offsets bounded by [-mask/2, mask/2), zero mask -> exact
    /// axis, and exactly 6 RNG bytes consumed per spawn.
    #[test]
    fn motherrnd_offsets_within_mask() {
        let mut v = vec![0xFF];
        let entry = v.len() as u16;
        v.push(mop::RND);
        v.extend_from_slice(&500u16.to_le_bytes());
        v.extend_from_slice(&1024u16.to_le_bytes()); // x mask
        v.extend_from_slice(&2048u16.to_le_bytes()); // y mask
        v.extend_from_slice(&0u16.to_le_bytes()); // z mask
        v.extend_from_slice(&99u16.to_le_bytes());
        v.extend_from_slice(&[0, 0, 0]); // unresolved strat -> inert child
        v.push(mop::GOTO);
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&entry.to_le_bytes());

        let (mut g, m) = game_with_mother(entry);
        g.vars.rng = [0x12, 0x34, 0x56, 0x78];
        let rng_before = g.vars.rng;
        bemother_on(&mut g, m, &v);
        let ci = g
            .objs
            .active_indices()
            .into_iter()
            .find(|&i| g.objs.aliens[i as usize].shape == 99)
            .expect("child spawned");
        let c = &g.objs.aliens[ci as usize];
        let dx = (c.worldx - 5) as i32;
        let dy = (c.worldy + 5) as i32;
        assert!((-512..512).contains(&dx), "dx {dx}");
        assert!((-1024..1024).contains(&dy), "dy {dy}");
        assert_eq!(c.worldz, 4000, "zero z mask -> mother z exactly");
        assert_ne!(g.vars.rng, rng_before, "RNG stream advanced");

        // Replay with the same seed: byte-for-byte deterministic offsets.
        let (mut g2, m2) = game_with_mother(entry);
        g2.vars.rng = [0x12, 0x34, 0x56, 0x78];
        bemother_on(&mut g2, m2, &v);
        let ci2 = g2
            .objs
            .active_indices()
            .into_iter()
            .find(|&i| g2.objs.aliens[i as usize].shape == 99)
            .unwrap();
        assert_eq!(
            (
                g2.objs.aliens[ci2 as usize].worldx,
                g2.objs.aliens[ci2 as usize].worldy
            ),
            (c.worldx, c.worldy)
        );
    }

    /// motherloop runs its body `count` times total, then falls through;
    /// motherend removes the mother.
    #[test]
    fn motherloop_and_motherend() {
        // [obj count=0 shape=7][loop -> obj, count=3][end]
        let mut v = vec![0xFF];
        let entry = v.len() as u16;
        v.push(mop::OBJ);
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // x,y,z = 0
        v.extend_from_slice(&7u16.to_le_bytes());
        v.extend_from_slice(&[0, 0, 0]);
        v.push(mop::LOOP);
        v.extend_from_slice(&0u16.to_le_bytes());
        v.extend_from_slice(&entry.to_le_bytes());
        v.push(3); // ml_count
        v.push(mop::END);

        let (mut g, m) = game_with_mother(entry);
        bemother_on(&mut g, m, &v);
        // ROM: body executes, loop jumps back while sbyte3+1 < 3 -> body
        // runs 3 times total, then END fires on the same tick.
        assert_eq!(count_shape(&g, 7), 3);
        assert_eq!(g.objs.aldead, 1, "motherend removed the mother");
    }

    /// mother1_istrat pins the mother's Z to player Z + initial delta every
    /// tick and drives bemother off the real MOTHERS.ASM blob.
    #[test]
    fn mother1_follows_player_and_spawns_from_blob() {
        let mm = sf_map::mothers::mother_maps();
        let (mut g, m) = game_with_mother(mm.mother_1);
        // delta = 4000 - 1000 = 3000.
        strat_mother1_init(&mut g, m);
        let before = g.objs.active_indices().len();
        assert_eq!(g.objs.aliens[m as usize].worldz, 4000);
        assert!(before >= 3, "mother_1 spawned its first child");
        // Child is an asteroid1 proxy with the slowmeteor strat resolved.
        let ci = g
            .objs
            .active_indices()
            .into_iter()
            .find(|&i| g.objs.aliens[i as usize].shape == 275)
            .expect("asteroid child");
        assert!(
            g.objs.aliens[ci as usize].stratptr.is_some(),
            "slowmeteor strategy registered at STRAT_ADDR_SLOWMETEOR"
        );
        // Move the player; the tick re-pins Z.
        g.objs.aliens[0].worldz = 1500;
        strat_mother1_tick(&mut g, m);
        assert_eq!(g.objs.aliens[m as usize].worldz, 4500);
    }

    /// The full loop: mother1 on mother_0 keeps producing children as the
    /// waits elapse (150-unit cadence at lastzchange=65).
    #[test]
    fn mother0_wave_cadence() {
        let mm = sf_map::mothers::mother_maps();
        let (mut g, m) = game_with_mother(mm.mother_0);
        strat_mother1_init(&mut g, m);
        let mut spawned = count_shape(&g, 275);
        assert_eq!(spawned, 1);
        // 150 / 65 -> a new child every 2-3 ticks, forever (goto loop).
        for _ in 0..20 {
            strat_mother1_tick(&mut g, m);
        }
        spawned = count_shape(&g, 275);
        assert!(
            (7..=10).contains(&spawned),
            "20 ticks at lzc=65 vs 150-unit waits -> ~8-9 children, got {spawned}"
        );
    }

    /// The address-collision regression: STRAT_ADDR_MOTHER1 must NOT
    /// resolve to the same strategy as synthetic istrat 0 (the player).
    #[test]
    fn mother_addr_does_not_collide_with_player() {
        let mut g = Game::new();
        crate::table::register_all(&mut g);
        let mother = g.world.find_strategy_address(STRAT_ADDR_MOTHER1);
        let player = g.world.find_strategy_address(0x020000);
        assert!(mother.is_some());
        assert!(player.is_some());
        assert_ne!(mother, player, "mothers used to run the player strat");
    }
}
