//! Tick 178–179: boost_Istrat / boost_strat pitch+yaw Roffs + call-site wiring.

use sf_game::alien::{ObjectVisualKind, ASF2_COLLDISABLE, ASF3_REALOBJ, ASF_INVISIBLE, NUMBER_AL};
use sf_game::Game;
use sf_strat::common::{boost_istrat, boost_sprite, boost_strat, set_boost_zoff, sv, StratRam};
use sf_strat::enemy_a::{shipoutoflb3_istrat, shipoutoflb3_strat};
use sf_strat::player::set_player_out_of_lb2a;
use sf_strat::snes_trig::strat_roffs_pitch_yaw;

const BOOST_SPRITE_SIZE: u8 = 10;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].sflags3 |= ASF3_REALOBJ;
    idx
}

fn count_active(g: &Game) -> usize {
    (0..NUMBER_AL).filter(|&i| g.objs.aliens[i].active).count()
}

#[test]
fn boost_istrat_falls_through_to_first_attached_tick() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.set_sv_i16(sv::BOOSTOBJ, idx as i16);
    set_boost_zoff(&mut g, -30);
    g.objs.aliens[idx as usize].sflags |= ASF_INVISIBLE;
    g.objs.aliens[idx as usize].sbyte1 = BOOST_SPRITE_SIZE;
    boost_istrat(&mut g, idx);
    let al = &g.objs.aliens[idx as usize];
    assert_eq!(al.count, 9);
    assert_ne!(al.sflags2 & ASF2_COLLDISABLE, 0);
    assert_eq!(al.sflags & ASF_INVISIBLE, 0);
    assert!(al.stratptr.is_some());
    assert_eq!(al.visual_kind, ObjectVisualKind::ScaledSprite);
    assert_eq!(al.depthoffset, 0);
    assert_eq!(
        al.tx,
        BOOST_SPRITE_SIZE - 1,
        "source size operand is copied into al_tx before the first tick"
    );
}

#[test]
fn boost_strat_parks_with_pitch_yaw_zoff() {
    let mut g = Game::new();
    let host = spawn(&mut g);
    g.objs.aliens[host as usize].worldx = 100;
    g.objs.aliens[host as usize].worldy = -40;
    g.objs.aliens[host as usize].worldz = 500;
    g.objs.aliens[host as usize].rotx = 0;
    g.objs.aliens[host as usize].roty = 0;
    g.vars.set_sv_i16(sv::BOOSTOBJ, host as i16);
    set_boost_zoff(&mut g, -30);

    let flame = boost_sprite(&mut g, None).expect("flame");
    g.run_strategies();

    let (rx, ry, rz) = strat_roffs_pitch_yaw(0, 0, 0, 0, -30);
    let al = &g.objs.aliens[flame as usize];
    assert_eq!(al.worldx, 100i16.wrapping_add(rx));
    assert_eq!(al.worldy, (-40i16).wrapping_add(ry));
    assert_eq!(al.worldz, 500i16.wrapping_add(rz));
    assert_eq!(al.count, 9); // lifecnt dec
}

#[test]
fn boost_strat_removes_when_host_gone() {
    let mut g = Game::new();
    let flame = spawn(&mut g);
    boost_istrat(&mut g, flame);
    g.vars.set_sv_i16(sv::BOOSTOBJ, -1);
    g.objs.aldead = 0;
    boost_strat(&mut g, flame);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn boost_strat_expires_after_ten_ticks() {
    let mut g = Game::new();
    let host = spawn(&mut g);
    g.vars.set_sv_i16(sv::BOOSTOBJ, host as i16);
    set_boost_zoff(&mut g, -30);
    let flame = boost_sprite(&mut g, Some(BOOST_SPRITE_SIZE)).expect("flame");
    assert_eq!(g.objs.aliens[flame as usize].sbyte1, BOOST_SPRITE_SIZE);
    g.run_strategies();
    assert_eq!(g.objs.aliens[flame as usize].tx, 9);
    for _ in 1..10 {
        g.objs.aldead = 0;
        boost_strat(&mut g, flame);
    }
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn lb2a_spawns_boost_flame() {
    let mut g = Game::new();
    let p = spawn(&mut g);
    let before = count_active(&g);
    set_player_out_of_lb2a(&mut g, p);
    assert_eq!(g.vars.sv_i16(sv::BOOSTOBJ), p as i16);
    assert_eq!(g.vars.sv_u8(sv::BOOSTZOFF) as i8, -30);
    assert!(count_active(&g) > before, "boost_sprite must spawn flame");
}

#[test]
fn shipoutoflb3_boost_sets_zoff_neg80() {
    let mut g = Game::new();
    let view = spawn(&mut g);
    g.vars.set_sv_i16(sv::VIEWTOOBJ, view as i16);
    let idx = spawn(&mut g);
    shipoutoflb3_istrat(&mut g, idx);
    // Jump to state 3 with sbyte2=1 so next tick fires boost.
    g.objs.aliens[idx as usize].stratstate = 3;
    g.objs.aliens[idx as usize].sbyte2 = 1;
    let before = count_active(&g);
    shipoutoflb3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].stratstate, 4);
    assert_eq!(g.vars.sv_u8(sv::BOOSTZOFF) as i8, -80);
    assert!(count_active(&g) > before);
    // Authored flame size from boost_sprite #10.
    let flame = (0..NUMBER_AL)
        .find(|&i| g.objs.aliens[i].active && i != idx as usize && i != view as usize)
        .expect("flame");
    assert_eq!(g.objs.aliens[flame].sbyte1, BOOST_SPRITE_SIZE);
    boost_istrat(&mut g, flame as u16);
    assert_eq!(
        g.objs.aliens[flame].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
    assert_eq!(g.objs.aliens[flame].tx, BOOST_SPRITE_SIZE - 1);
}
