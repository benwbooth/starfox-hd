//! Tick 125: planet yvel125 dispatch + steer ztilt ground/wall gates.

use sf_core::pad;
use sf_game::vars::{PFM_WOBBLE, SPACE_MODE};
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::player::{set_player_on_planet, strat_player, strat_spawn_player};

const PFM_DIEFALL: u8 = 1;
const PFM_DIEYROT: u8 = 2;
const PML_LWLEFT: u8 = 1;
const PML_RWRIGHT: u8 = 2;
const PML_BBOTTOM: u8 = 128;

fn set_pad(g: &mut Game, pad: u16) {
    let prev = g.vars.pad1;
    g.vars.lastcont0 = (prev >> 8) as u8;
    g.vars.lastcontl0 = (prev & 0xFF) as u8;
    g.vars.pad1 = pad;
}

fn ready_player(g: &mut Game) -> u16 {
    let idx = strat_spawn_player(g).expect("player");
    g.vars.set_sv_u8(sv::STAYBLACK, (-1i8) as u8);
    g.vars.set_sv_u8(sv::DOINGWIPE, 0);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, 0xFF);
    idx
}

#[test]
fn planet_init_uses_yvel125_despite_diefall_bits() {
    // High #3: ROM-faithful planet_flymode sets PFM_DIEFALL|DIEYROT, but the
    // strat still runs do_player_Yvel125 (×1.375), not yvel_d2 / limit_x.
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    assert_eq!(g.vars.game_mode, SPACE_MODE, "spawn defaults to space");
    set_player_on_planet(&mut g, idx);
    assert_eq!(g.vars.game_mode, 0, "planet init must clear SPACE_MODE");
    assert!(g.vars.playerflymode & PFM_DIEFALL != 0);
    assert!(g.vars.playerflymode & PFM_DIEYROT != 0);
    assert!(g.vars.playerflymode & PFM_WOBBLE != 0);

    // Seed pitch so gen_3dvecs produces motion; yvel125 path must move.
    g.vars.set_sv_i16(sv::PLROTX, 0x2000);
    g.objs.aliens[idx as usize].vel = 40;
    let y0 = g.objs.aliens[idx as usize].worldy;
    set_pad(&mut g, pad::UP);
    for _ in 0..8 {
        strat_player(&mut g, idx);
    }
    let dy = (g.objs.aliens[idx as usize].worldy as i32 - y0 as i32).abs();
    assert!(dy > 0, "planet yvel125 path must move on pitch (dy={dy})");

    // ×1.375 formula unit — vy=80 → 80+20+10=110.
    let vy = 80i16;
    assert_eq!(vy.wrapping_add(vy >> 2).wrapping_add(vy >> 3), 110);
}

#[test]
fn dpad_ztilt_skipped_near_floor_and_wing_wall() {
    let mut g = Game::new();
    let idx = ready_player(&mut g);
    set_player_on_planet(&mut g, idx);
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 0);
    g.vars.set_sv_u8(sv::PMOVELIMIT, 0);
    g.vars.set_sv_u8(sv::PMOVELIMITAND, PML_BBOTTOM);
    g.vars.set_sv_i16(sv::MAXPMOVEY, 0);
    // Far from floor → LEFT adds deg45/15 (=3).
    g.objs.aliens[idx as usize].worldy = -100;
    set_pad(&mut g, pad::LEFT);
    strat_player(&mut g, idx);
    let z_ok = g.vars.sv_u8(sv::PLAYER_ZTILT) as i8;
    assert!(z_ok > 0, "open air LEFT should bank, got {z_ok}");

    // Near floor (worldy >= maxY-30) with Bbottom armed → no ztilt add.
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 0);
    g.objs.aliens[idx as usize].worldy = -20; // maxY=0 → threshold -30; -20 is lower
    set_pad(&mut g, 0);
    set_pad(&mut g, pad::LEFT);
    strat_player(&mut g, idx);
    assert_eq!(
        g.vars.sv_u8(sv::PLAYER_ZTILT) as i8,
        0,
        "near floor must skip dpad ztilt"
    );

    // Wing against left wall → skip LEFT ztilt.
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 0);
    g.objs.aliens[idx as usize].worldy = -100;
    g.vars.set_sv_u8(sv::PMOVELIMIT, PML_LWLEFT);
    set_pad(&mut g, 0);
    set_pad(&mut g, pad::LEFT);
    strat_player(&mut g, idx);
    assert_eq!(
        g.vars.sv_u8(sv::PLAYER_ZTILT) as i8,
        0,
        "pml_lwleft must skip LEFT ztilt"
    );

    // RIGHT + pml_rwright similarly.
    g.vars.set_sv_u8(sv::PLAYER_ZTILT, 0);
    g.vars.set_sv_u8(sv::PMOVELIMIT, PML_RWRIGHT);
    set_pad(&mut g, 0);
    set_pad(&mut g, pad::RIGHT);
    strat_player(&mut g, idx);
    assert_eq!(
        g.vars.sv_u8(sv::PLAYER_ZTILT) as i8,
        0,
        "pml_rwright must skip RIGHT ztilt"
    );
}
