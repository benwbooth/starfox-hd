//! Ground object strategies (static terrain, staying-relative objects).
//!
//! C oracle: `src/strat/strat_ground.c` (decompiled from GSTRATS.ASM
//! stayrel/stayrelhard180yr/staydist and GASTRATS.ASM gnd).

use crate::enemy_a::{pviewposz, sid, DEG180};
use sf_game::alien::{ASF_COLLDISABLE, ATGND};
use sf_game::game::Game;
use sf_game::vars::{HARD_AP, HARD_HP};

/// C `stayrel_strat` (GSTRATS.ASM:699-703): scroll with the world and keep
/// collision disabled.
fn stayrel_strat(g: &mut Game, idx: u16) {
    let v = g.vars.pviewvelz;
    let al = &mut g.objs.aliens[idx as usize];
    // s_add_playerZ x
    al.worldz = al.worldz.wrapping_add(v);
    // s_set_alsflag x,colldisable
    al.sflags |= ASF_COLLDISABLE;
}

/// C `Strat_StayRel_Init` (src/strat/strat_ground.c:27).
pub fn strat_stayrel_init(g: &mut Game, idx: u16) {
    let s = sid(g, stayrel_strat);
    g.objs.aliens[idx as usize].stratptr = Some(s);
}

/// C `Strat_Gnd_Init` (src/strat/strat_ground.c:36, GASTRATS.ASM:3720-3725):
/// static ground-plane segment for the scramble tunnel maps.
pub fn strat_gnd_init(g: &mut Game, idx: u16) {
    let al = &mut g.objs.aliens[idx as usize];
    // s_set_alptrs x,0,0,0
    al.stratptr = None;
    al.collstratptr = None;
    al.expstratptr = None;
    // s_set_altype x,gnd
    al.type_ |= ATGND;
    // s_set_alsflag x,colldisable
    al.sflags |= ASF_COLLDISABLE;
}

/// C `stayrelhard180yr_strat` (GSTRATS.ASM:692-695).
fn stayrelhard180yr_strat(g: &mut Game, idx: u16) {
    let v = g.vars.pviewvelz;
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.worldz.wrapping_add(v);
}

/// C `Strat_StayRelHard180yr_Init` (src/strat/strat_ground.c:58,
/// GSTRATS.ASM:688-691).
pub fn strat_stayrelhard180yr_init(g: &mut Game, idx: u16) {
    let s = sid(g, stayrelhard180yr_strat);
    let al = &mut g.objs.aliens[idx as usize];
    // s_hardvars x
    al.hp = HARD_HP;
    al.ap = HARD_AP;
    // s_set_alvar B,x,al_roty,#deg180
    al.roty = DEG180;
    al.stratptr = Some(s);
}

/// C `Strat_StayDist_Init` (src/strat/strat_ground.c:75,
/// GSTRATS.ASM:706-711): init-only, `sword1` is the desired Z offset from
/// `pviewposz`.
pub fn strat_staydist_init(g: &mut Game, idx: u16) {
    let pvz = pviewposz(g);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldz = al.sword1.wrapping_add(pvz);
    al.sflags |= ASF_COLLDISABLE;
    al.stratptr = None;
}
