//! THEEND letter-flip / zoom / fin / flyaway (KSTRATS.ASM:896–1089).
//!
//! Credits “THE END” letters: zoom in, settle at 0°/180°, flip on hit
//! (spawn distraction + tumble), then fly away when all six are OK.

use sf_game::alien::{StratId, ASF_COLLDISABLE, ATZREMOVE};
use sf_game::game::{Game, StrategyFn};

use crate::common::{add_player_z, sf_random, strat_make_obj};
use crate::enemy_a::boss_keeprel_to_player;

/// ISTRATS indices used by `theend_flip` distraction spawns.
const IS_SZACO0: u8 = 129;
const IS_TADPOLE: u8 = 227;
const IS_SZACO5: u8 = 155;
const IS_MISSPOD: u8 = 67;

/// Shape ids (sf-map / ISTRATS pairing).
const SH_ZACO_4: u16 = 105; // paired with szaco0
const SH_TADPOLE: u16 = 227;
const SH_ZACO_B: u16 = 201;
const SH_BIG_M: u16 = 18;

fn sid(g: &mut Game, f: StrategyFn) -> StratId {
    g.world.register_strategy(f)
}

fn keeprel(g: &mut Game, idx: u16) {
    boss_keeprel_to_player(g, idx);
}

fn copy_pos(g: &mut Game, dst: u16, src: u16) {
    let s = g.objs.aliens[src as usize];
    let d = &mut g.objs.aliens[dst as usize];
    d.worldx = s.worldx;
    d.worldy = s.worldy;
    d.worldz = s.worldz;
}

fn assign_istrat_if_present(g: &mut Game, idx: u16, istrat: u8) {
    if let Some(id) = g.world.istrats[istrat as usize] {
        g.objs.aliens[idx as usize].stratptr = Some(id);
    }
}

/// ROM `theend_zoom_istrat` (KSTRATS.ASM:896).
pub fn theend_zoom_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, theend_zoom_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s);
    al.rotx = 0;
    al.roty = 0;
    al.sbyte1 = 33;
    theend_zoom_strat(g, idx);
}

/// ROM `theend_zoom_strat` — spin + pull in, then fin.
pub fn theend_zoom_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.worldz = al.worldz.wrapping_sub(19);
    }
    add_player_z(g, idx);
    keeprel(g, idx);
    // s_decbeq: DEC then BEQ → fin
    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb == 0 {
        theend_fin_istrat(g, idx);
    }
}

/// ROM `theend_zoom2_istrat` (KSTRATS.ASM:911).
pub fn theend_zoom2_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, theend_zoom2_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s);
    al.rotx = 0;
    al.roty = 0;
    al.sbyte1 = 33;
    theend_zoom2_strat(g, idx);
}

/// ROM `theend_zoom2_strat` — same motion, handoff to fin2.
pub fn theend_zoom2_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.roty = al.roty.wrapping_add(4);
        al.worldz = al.worldz.wrapping_sub(19);
    }
    add_player_z(g, idx);
    keeprel(g, idx);
    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb == 0 {
        theend_fin2_istrat(g, idx);
    }
}

/// ROM `theend_check_istrat` (KSTRATS.ASM:927).
pub fn theend_check_istrat(g: &mut Game, idx: u16) {
    if g.vars.numendok == 6 {
        let s = sid(g, theend_check_wait);
        g.objs.aliens[idx as usize].stratptr = Some(s);
        g.hooks.play_music(6);
    }
    g.vars.numendok = 0;
    add_player_z(g, idx);
    keeprel(g, idx);
}

fn theend_check_wait(g: &mut Game, idx: u16) {
    g.vars.numendok = 0xFF; // -1
    add_player_z(g, idx);
    keeprel(g, idx);
}

/// ROM `theend_fin_istrat` — settle when rotz==0.
pub fn theend_fin_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, theend_fin_strat);
    let c = sid(g, theend_flip_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(c);
    al.type_ &= !ATZREMOVE; // s_setnoremove_behind
    theend_fin_strat(g, idx);
}

/// ROM `theend_fin_strat`.
pub fn theend_fin_strat(g: &mut Game, idx: u16) {
    if g.objs.aliens[idx as usize].rotz == 0 {
        theend_ok(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = al.sword2 as u16;
    }
    add_player_z(g, idx);
    keeprel(g, idx);
}

/// ROM `theend_fin2_istrat` — settle at rotz 0 or 180.
pub fn theend_fin2_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, theend_fin2_strat);
    let c = sid(g, theend_flip_istrat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(c);
    al.type_ &= !ATZREMOVE;
    theend_fin2_strat(g, idx);
}

/// ROM `theend_fin2_strat`.
pub fn theend_fin2_strat(g: &mut Game, idx: u16) {
    let rz = g.objs.aliens[idx as usize].rotz;
    if rz == 0 || rz == 128 {
        theend_ok(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = al.sword2 as u16;
    }
    add_player_z(g, idx);
    keeprel(g, idx);
}

/// ROM `theendok` — count letter or fly away when numendok already -1.
fn theend_ok(g: &mut Game, idx: u16) {
    if (g.vars.numendok as i8) < 0 {
        theend_flyaway_istrat(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.shape = al.sword1 as u16;
    }
    g.vars.numendok = g.vars.numendok.wrapping_add(1);
    add_player_z(g, idx);
    keeprel(g, idx);
}

/// ROM `theend_flyaway_istrat` (KSTRATS.ASM:996).
pub fn theend_flyaway_istrat(g: &mut Game, idx: u16) {
    let s = sid(g, theend_flyaway_strat);
    let al = &mut g.objs.aliens[idx as usize];
    al.stratptr = Some(s);
    al.collstratptr = Some(s);
    al.sflags |= ASF_COLLDISABLE;
    al.type_ |= ATZREMOVE; // s_setremove_behind
    theend_flyaway_strat(g, idx);
}

/// ROM `theend_flyaway_strat`.
pub fn theend_flyaway_strat(g: &mut Game, idx: u16) {
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(3);
        al.roty = al.roty.wrapping_add(2);
        al.rotz = al.rotz.wrapping_add(6);
        al.worldz = al.worldz.wrapping_sub(20);
    }
    add_player_z(g, idx);
}

/// ROM `theend_flip_istrat` (KSTRATS.ASM:1013) — hit reaction + tumble.
pub fn theend_flip_istrat(g: &mut Game, idx: u16) {
    // s_beqdec sbyte3: if was 0 → .do; else dec and .fail (still tumble)
    let sb3 = g.objs.aliens[idx as usize].sbyte3;
    if sb3 != 0 {
        g.objs.aliens[idx as usize].sbyte3 = sb3.wrapping_sub(1);
    } else {
        g.objs.aliens[idx as usize].sbyte3 = 2;
        theend_flip_spawn_distraction(g, idx);
    }

    // .fail path (always): push strat, arm tumble
    g.objs.aliens[idx as usize].tempstratptr = g.objs.aliens[idx as usize].stratptr;
    g.objs.aliens[idx as usize].sbyte1 = 32;
    g.objs.aliens[idx as usize].sflags |= ASF_COLLDISABLE;
    let s = sid(g, theend_flip_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
    g.objs.aliens[idx as usize].vy = -45;
    g.objs.aliens[idx as usize].vz = 45 * 4;
    g.objs.aliens[idx as usize].sbyte2 = (sf_random(&mut g.vars) & 0xFF) as u8;
    theend_flip_strat(g, idx);
}

fn theend_flip_spawn_distraction(g: &mut Game, idx: u16) {
    let r = (sf_random(&mut g.vars) & 0xFF) as u8;
    let (shape, istrat, dx, dy, dz, hp_ap) = if r < 90 {
        (SH_BIG_M, IS_MISSPOD, 0i16, 0i16, 3000i16, None)
    } else if r < 146 {
        (SH_ZACO_B, IS_SZACO5, -400, -400, 2000, Some((2u8, 6u8)))
    } else if r < 210 {
        (SH_TADPOLE, IS_TADPOLE, 500, -200, 3000, None)
    } else {
        (SH_ZACO_4, IS_SZACO0, -100, 1000, 3500, None)
    };
    let Some(y) = strat_make_obj(g, shape) else {
        return;
    };
    copy_pos(g, y, idx);
    {
        let al = &mut g.objs.aliens[y as usize];
        al.worldx = al.worldx.wrapping_add(dx);
        al.worldy = al.worldy.wrapping_add(dy);
        al.worldz = al.worldz.wrapping_add(dz);
        if let Some((hp, ap)) = hp_ap {
            al.hp = hp;
            al.ap = ap;
        }
    }
    assign_istrat_if_present(g, y, istrat);
}

/// ROM `theend_flip_strat` — tumble then restore pushed strat.
pub fn theend_flip_strat(g: &mut Game, idx: u16) {
    add_player_z(g, idx);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.rotx = al.rotx.wrapping_add(8);
        al.roty = al.roty.wrapping_add(8);
        let r = al.sbyte2;
        let add = if r < 86 {
            4u8
        } else if r < 172 {
            6
        } else {
            2
        };
        al.rotz = al.rotz.wrapping_add(add);
    }
    // s_decbeq sbyte1
    let sb = g.objs.aliens[idx as usize].sbyte1.wrapping_sub(1);
    g.objs.aliens[idx as usize].sbyte1 = sb;
    if sb == 0 {
        g.objs.aliens[idx as usize].stratptr = g.objs.aliens[idx as usize].tempstratptr;
        g.objs.aliens[idx as usize].sflags &= !ASF_COLLDISABLE;
        keeprel(g, idx);
        return;
    }
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldy = al.worldy.wrapping_add(al.vy);
        al.worldz = al.worldz.wrapping_add(al.vz);
        al.vy = al.vy.wrapping_add(3);
        al.vz = al.vz.wrapping_sub(3 * 4);
    }
    keeprel(g, idx);
}
