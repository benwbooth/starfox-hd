//! ROM pole0 spinner + cockdumpl/cockpit/out props (GA2STRAT / PSTRATS).

use sf_game::alien::{ASF_COLLDISABLE, ASF_HITFLASH};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{pole0_istrat, pole0_strat, pole0col_istrat};
use sf_strat::player::{
    cockdumpl_istrat, cockdumpl_strat, cockpit_istrat, cockpit_strat, cockpitout_istrat,
    cockpitout_strat, cockshipout_istrat, cockshipout_strat,
};

fn spawn_player(g: &mut Game, z: i16) -> u16 {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    p
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 1000;
    idx
}

#[test]
fn pole0_istrat_hard_and_spins() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    pole0_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    let s1 = g.objs.aliens[idx as usize].sbyte1 as i8;
    assert!(s1 == 3 || s1 == -3);
    let z0 = g.objs.aliens[idx as usize].worldz;
    let initial_roll = g.objs.aliens[idx as usize].rotz;
    pole0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_add(30));
    assert_eq!(
        g.objs.aliens[idx as usize].rotz,
        initial_roll.wrapping_add(g.objs.aliens[idx as usize].sbyte1)
    );
}

#[test]
fn pole0col_hf2_speeds_up_positive_spin() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    pole0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte1 = 3;
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].hitflags = 0x02; // HF2
    pole0col_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1 as i8, 5);
    // col sets sbyte2=6 then jmpto_strat → pole0_strat beqdec → 5.
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 5);
    assert_eq!(g.objs.aliens[idx as usize].hitflags, 0);
}

#[test]
fn pole0col_hf3_clears_positive_spin() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    pole0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte1 = 3;
    g.objs.aliens[idx as usize].sbyte2 = 0;
    g.objs.aliens[idx as usize].hitflags = 0x04; // HF3
    pole0col_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0);
}

#[test]
fn pole0col_debounced_while_sbyte2() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    pole0_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].sbyte1 = 3;
    g.objs.aliens[idx as usize].sbyte2 = 3;
    g.objs.aliens[idx as usize].hitflags = 0x02;
    pole0col_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1 as i8, 3); // unchanged
}

#[test]
fn cockdumpl_spawns_cockpit_when_player_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 20; // |dz|<50 vs player at 0
    cockdumpl_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    assert_eq!(g.objs.aliens[idx as usize].count, 8);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    cockdumpl_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before);
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & 0x10, 0); // sflag1
}

#[test]
fn cockpit_drifts_toward_camera() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    cockpit_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 5;
    let z0 = g.objs.aliens[idx as usize].worldz;
    cockpit_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_sub(10));
    assert_eq!(g.objs.aliens[idx as usize].count, 4);
}

#[test]
fn cockshipout_tracks_player_and_accelerates() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.vars.player_posx = 50;
    g.vars.player_posy = -30;
    cockshipout_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 19);
    assert_eq!(g.objs.aliens[idx as usize].sword1, 60);
    let z0 = g.objs.aliens[idx as usize].worldz;
    cockshipout_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 50);
    assert_eq!(g.objs.aliens[idx as usize].worldy, -30);
    assert_eq!(g.objs.aliens[idx as usize].sword1, 70);
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_add(60) // sword1 before +10
    );
}

#[test]
fn cockpitout_tracks_and_advances_z() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.vars.player_posx = 10;
    g.vars.player_posy = -20;
    cockpitout_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 8);
    let z0 = g.objs.aliens[idx as usize].worldz;
    cockpitout_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldx, 10);
    assert_eq!(g.objs.aliens[idx as usize].worldz, z0.wrapping_add(20));
}

#[test]
fn cockshipout_hitflash_when_dying() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    cockshipout_istrat(&mut g, idx);
    g.vars.pshipflags2 |= sf_game::vars::PSF2_PLAYERHP0;
    g.vars.set_sv_u8(sv::PLAYER_ZSTRATADD, 40);
    g.vars.gameframe = 0; // even → hitflash
    cockshipout_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotz, 40);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_HITFLASH, 0);
}
