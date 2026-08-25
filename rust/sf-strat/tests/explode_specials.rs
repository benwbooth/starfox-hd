//! Tick 148: explode `s_test_special` — special OR Cspecial → specials_dead++;
//! never decrement specialobjtotal / never set GF_BOSSDEAD (AUDIT_HUD Critical #3).

use sf_game::alien::{ASF4_CSPECIAL, ASF_SPECIAL};
use sf_game::vars::GF_BOSSDEAD;
use sf_game::Game;
use sf_strat::enemy_a::{strat_explode, wm};

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].flags |= 0x10; // AF_INVIEW_PL (init_objvars seed)
    idx
}

#[test]
fn explode_counts_plain_special_into_specials_dead() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sflags |= ASF_SPECIAL;
    g.objs.aliens[idx as usize].sflags2 |= 0x08; // ASF2_NOEXPSND — quiet
    g.world.specialobjtotal = 3;
    g.vars.write_ext8(wm::SPECIALS_DEAD, 0);
    g.vars.gameflags = 0;

    strat_explode(&mut g, idx);

    assert_eq!(g.vars.read_ext8(wm::SPECIALS_DEAD), 1);
    assert_eq!(
        g.world.specialobjtotal, 3,
        "ROM never decrements specialobjtotal"
    );
    assert_eq!(
        g.vars.gameflags & GF_BOSSDEAD,
        0,
        "special kill must not set GF_BOSSDEAD"
    );
}

#[test]
fn explode_counts_cspecial_into_specials_dead() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sflags4 |= ASF4_CSPECIAL;
    g.objs.aliens[idx as usize].sflags2 |= 0x08;
    g.world.specialobjtotal = 1;
    g.vars.write_ext8(wm::SPECIALS_DEAD, 5);

    strat_explode(&mut g, idx);

    assert_eq!(g.vars.read_ext8(wm::SPECIALS_DEAD), 6);
    assert_eq!(g.world.specialobjtotal, 1);
    assert_eq!(g.vars.gameflags & GF_BOSSDEAD, 0);
}

#[test]
fn explode_non_special_leaves_counters() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.objs.aliens[idx as usize].sflags2 |= 0x08;
    g.world.specialobjtotal = 2;
    g.vars.write_ext8(wm::SPECIALS_DEAD, 0);

    strat_explode(&mut g, idx);

    assert_eq!(g.vars.read_ext8(wm::SPECIALS_DEAD), 0);
    assert_eq!(g.world.specialobjtotal, 2);
}

/// Unified lives store: wm::LIVES == sv::LIVES (0x0520).
#[test]
fn lives_wram_slot_is_rom_0520() {
    assert_eq!(wm::LIVES, 0x0520);
    let mut g = Game::new();
    g.vars.write_ext8(wm::LIVES, 3);
    assert_eq!(g.vars.read_ext8(0x0520), 3);
    g.vars.write_ext8(0x0520, 5);
    assert_eq!(g.vars.read_ext8(wm::LIVES), 5);
}
