//! ROM exitlight blink + fastfighter 1/2/3 + blackholeexit alias (GASTRATS / KSTRATS).

use sf_game::alien::ASF_COLLDISABLE;
use sf_game::Game;
use sf_strat::bosses::blackholeexit_istrat;
use sf_strat::enemy_a::{
    exitlight1_istrat, exitlight2_istrat, exitlight6_istrat, exitlight_a_strat, exitlight_b_strat,
    exitlight_init, fastfighter1_istrat, fastfighter1_strat, fastfighter2_istrat,
    fastfighter2_strat, fastfighter3_istrat, fastfighter3_strat, fastfighter_init, DEG180,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 3000;
    g.objs.aliens[idx as usize].worldy = -80;
    idx
}

#[test]
fn exitlight1_cycles_a_then_b() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    exitlight1_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    // istrat set sbyte2=2 then A_init ran one A tick (beqdec → sbyte2=1, colanim 1).
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 1);
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 1);
    // Burn remaining A delay.
    exitlight_a_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 0);
    exitlight_a_strat(&mut g, idx); // sbyte2==0 → B_init
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 3);
    assert_eq!(g.objs.aliens[idx as usize].colframe & 0x7F, 0);
    // Burn B wait → back to A with sbyte2=12.
    g.objs.aliens[idx as usize].sbyte1 = 0;
    exitlight_b_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte2, 11); // 12 then one A tick
}

#[test]
fn exitlight_variants_set_stagger() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let a = spawn_obj(&mut g);
    let b = spawn_obj(&mut g);
    let c = spawn_obj(&mut g);
    exitlight2_istrat(&mut g, a);
    exitlight6_istrat(&mut g, b);
    exitlight_init(&mut g, c); // sbyte2 untouched (0) → immediate B on first A tick
                               // After istrat: sbyte2 was N, A tick decremented once.
    assert_eq!(g.objs.aliens[a as usize].sbyte2, 3); // 4-1
    assert_eq!(g.objs.aliens[b as usize].sbyte2, 11); // 12-1
                                                      // c started with 0 → A_init → A_strat → B_init
    assert_eq!(g.objs.aliens[c as usize].sbyte1, 3);
}

#[test]
fn fastfighter_init_sets_stats_and_vecs() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fastfighter1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    assert_eq!(g.objs.aliens[idx as usize].vel, 80);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 8);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);
    // gen_3dvecs from deg180 should produce non-zero vz.
    assert_ne!(g.objs.aliens[idx as usize].vz, 0);
}

#[test]
fn fastfighter1_sprays_when_far_from_ptr() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fastfighter1_istrat(&mut g, idx);
    // No ptr → spray path. Force fire gate.
    g.vars.gameframe = 0; // (0+idx)&7 — idx is 1 typically
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    // Align phase: need (gf+idx)&7==0.
    let phase = idx as u8;
    g.vars.gameframe = (0u8.wrapping_sub(phase) as u16) & 0xff;
    fastfighter1_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after >= before); // may spawn laser
}

#[test]
fn fastfighter1_aims_at_ptr_when_close() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fastfighter1_istrat(&mut g, idx);
    let tgt = spawn_obj(&mut g);
    g.objs.aliens[tgt as usize].worldz = 500;
    g.objs.aliens[idx as usize].worldz = 1000; // |dz|=500 < 1500; tgt behind (z smaller)
    g.objs.aliens[idx as usize].ptr = tgt;
    g.objs.aliens[idx as usize].sbyte1 = 3;
    // Fire gate: (gameframe + strat_phase_offset(idx)) & 3 == 0, where the
    // retail pool-phase stagger is seed 54 + step 54 per slot.
    // strat_phase_offset: seed 54 + step 54 per slot (u8 wrap).
    let phase = 54u8.wrapping_add(54u8.wrapping_mul(idx as u8));
    g.vars.gameframe = (0u16.wrapping_sub(u16::from(phase))) & 3;
    fastfighter1_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 2);
}

#[test]
fn fastfighter3_fires_then_switches_to_fighter1() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fastfighter3_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 3);
    let s3 = g.objs.aliens[idx as usize].stratptr;
    g.objs.aliens[idx as usize].sbyte1 = 1;
    fastfighter3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 3); // reloaded
    assert_ne!(g.objs.aliens[idx as usize].stratptr, s3); // now fighter1
}

#[test]
fn fastfighter2_init_and_tick() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fastfighter2_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 4);
    let phase = idx as u8;
    g.vars.gameframe = (0u8.wrapping_sub(phase) as u16) & 0xff;
    fastfighter2_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}

#[test]
fn dofighter_spins_when_damaged() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    fastfighter_init(&mut g, idx);
    g.objs.aliens[idx as usize].hp = 3; // <= fighterHP-1
    g.objs.aliens[idx as usize].stratstate = 0;
    g.vars.gameframe = 0; // notdelay 1 fires
    let rotz0 = g.objs.aliens[idx as usize].rotz;
    // Call via fighter2 strat (always ends in dofighter).
    let tick_before = g.objs.aliens[idx as usize].stratstate;
    fastfighter2_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].hp = 3;
    g.objs.aliens[idx as usize].stratstate = 0;
    g.vars.gameframe = 0;
    // Force far so bank doesn't dominate; just spin path.
    g.objs.aliens[idx as usize].worldz = 5000;
    g.objs.aliens[0].worldz = 0;
    fastfighter2_strat(&mut g, idx);
    // next_state #2 from 0 → 1, then rotz -= 5.
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[idx as usize].rotz, rotz0.wrapping_sub(5));
    let _ = tick_before;
}

#[test]
fn blackholeexit_istrat_arms_draw_in() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].sbyte2 = 1; // le_bhole1
    blackholeexit_istrat(&mut g, idx);
    // init sets sbyte1=8, sbyte3=10 then first strat tick: sbyte1→7.
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 7);
    assert_eq!(g.objs.aliens[idx as usize].sbyte3, 10);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
}
