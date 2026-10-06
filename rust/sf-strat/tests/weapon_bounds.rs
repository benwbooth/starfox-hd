//! Source GSTRATS.ASM missile bounds: vertical equality and sequential gates.

use sf_game::alien::ASF2_COLLDISABLE;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::missbound_chk_exp;

#[test]
fn top_gates_intersect_and_use_the_scene_player_not_slot_zero() {
    let mut game = Game::new();
    let decoy = game.objs.alloc().unwrap();
    let player = game.objs.alloc().unwrap();
    let missile = game.objs.alloc().unwrap();
    game.vars.player_object = player as i16;
    game.vars.minpmove_y = -100;
    game.vars.set_sv_i16(sv::MISSBTOPLEFT, -100);
    game.vars.set_sv_i16(sv::MISSBTOPRIGHT, 100);
    game.vars.set_sv_u8(sv::MISSBOUNDFLAGS, 32 | 64);
    game.objs.aliens[decoy as usize].worldx = 2000;
    for player_x in [-101, -100, -99, 0, 99, 100, 101] {
        game.objs.aliens[player as usize].worldx = player_x;
        for y in [-101, -100, -99] {
            let al = &mut game.objs.aliens[missile as usize];
            al.hp = 5;
            al.sflags2 = 0x54;
            al.worldy = y;
            missbound_chk_exp(&mut game, missile);
            let killed = y <= -100 && player_x > -100 && player_x <= 100;
            let al = &game.objs.aliens[missile as usize];
            assert_eq!(al.hp, if killed { 0 } else { 5 });
            assert_eq!(al.sflags2, 0x54 | if killed { ASF2_COLLDISABLE } else { 0 });
            assert!(al.active);
            assert_eq!(game.objs.aldead, 0);
        }
    }
}

#[test]
fn left_bottom_gate_uses_strict_player_comparison_but_inclusive_vertical_edge() {
    let mut game = Game::new();
    let player = game.objs.alloc().unwrap();
    let missile = game.objs.alloc().unwrap();
    game.vars.set_sv_u8(sv::MISSBOUNDFLAGS, 16);
    game.vars.set_sv_i16(sv::MISSBBOTLEFT, 100);
    game.vars.set_sv_i16(sv::MAXMMOVEY, 200);
    for player_x in [99, 100, 101] {
        game.objs.aliens[player as usize].worldx = player_x;
        for y in [199, 200, 201] {
            let al = &mut game.objs.aliens[missile as usize];
            al.hp = 5;
            al.sflags2 = 0;
            al.worldy = y;
            missbound_chk_exp(&mut game, missile);
            assert_eq!(
                game.objs.aliens[missile as usize].hp == 0,
                player_x > 100 && y >= 200
            );
        }
    }
}

#[test]
fn source_boundary_comparisons_preserve_word_wrap() {
    let mut game = Game::new();
    let missile = game.objs.alloc().unwrap();
    game.vars.set_sv_i16(sv::MINMMOVEX, -200);
    game.vars.set_sv_i16(sv::MAXMMOVEX, 200);
    for (flags, x, killed) in [
        (1, -201, true),
        (1, -200, false),
        (1, i16::MAX, true),
        (2, 201, true),
        (2, 200, false),
        (2, i16::MIN, true),
    ] {
        game.vars.set_sv_u8(sv::MISSBOUNDFLAGS, flags);
        let al = &mut game.objs.aliens[missile as usize];
        al.hp = 5;
        al.sflags2 = 0;
        al.worldx = x;
        missbound_chk_exp(&mut game, missile);
        assert_eq!(game.objs.aliens[missile as usize].hp == 0, killed);
    }
}
