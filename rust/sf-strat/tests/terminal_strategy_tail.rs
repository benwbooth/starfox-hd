//! STRATMAC.INC s_dec_lifecnt x,1 marks death without ending the strategy.

use sf_core::pad;
use sf_game::alien::{ASF2_COLLDISABLE, ASF4_RELEXPLODE};
use sf_game::vars::PFM_DIEFALL;
use sf_game::Game;
use sf_strat::enemy_a::{explode_end, exppiece_strat, nuke_strat, ASF2_SFLAG1, ASF2_SMFLAG1};

#[test]
fn debris_moves_and_scrolls_on_the_visit_that_marks_death() {
    let mut game = Game::new();
    let id = game.objs.alloc().unwrap();
    game.vars.pviewvelz = 65;
    for count in u8::MIN..=u8::MAX {
        for relative in [false, true] {
            let object = &mut game.objs.aliens[id as usize];
            object.count = count;
            object.hp = 9;
            object.sflags2 = ASF2_SMFLAG1;
            object.sflags4 = if relative { ASF4_RELEXPLODE } else { 0 };
            [object.worldx, object.worldy, object.worldz] = [32760, -32766, 32765];
            [object.vx, object.vy, object.vz] = [10, -4, 7];
            [object.rotx, object.rotz] = [250, 254];

            exppiece_strat(&mut game, id);

            let object = game.objs.aliens[id as usize];
            assert_eq!(object.count, count.wrapping_sub(1));
            assert_eq!(object.hp, if count == 1 { 0 } else { 9 });
            assert_eq!(object.sflags2 & ASF2_COLLDISABLE != 0, count == 1);
            assert_eq!((object.rotx, object.rotz), (2, 2));
            let scroll = if relative { 65 - 65 / 2 } else { 0 };
            assert_eq!(
                [object.worldx, object.worldy, object.worldz],
                [
                    32760_i16.wrapping_add(10),
                    (-32766_i16).wrapping_sub(4),
                    32765_i16.wrapping_add(7).wrapping_add(scroll)
                ],
            );
            assert!(object.active, "death is not immediate slot retirement");
            assert_eq!(game.objs.aldead, 0);
        }
    }
}

#[test]
fn nuke_fall_acceleration_survives_lifetime_death_and_manual_detonation() {
    let mut game = Game::new();
    let id = game.objs.alloc().unwrap();
    game.vars.playerflymode = PFM_DIEFALL;
    for count in u8::MIN..=u8::MAX {
        for manual_detonation in [false, true] {
            game.vars.pad1 = if manual_detonation { pad::A } else { 0 };
            let object = &mut game.objs.aliens[id as usize];
            object.count = count;
            object.hp = 9;
            object.sflags2 = ASF2_SFLAG1; // Skip the independently tested boundary tail.
            [object.worldx, object.worldy, object.worldz] = [10, 20, 30];
            [object.vx, object.vy, object.vz] = [3, 32767, 5];

            nuke_strat(&mut game, id);

            let object = game.objs.aliens[id as usize];
            let dead = count == 1 || manual_detonation;
            assert_eq!(object.count, count.wrapping_sub(1));
            assert_eq!(object.hp, if dead { 0 } else { 9 });
            assert_eq!(object.sflags2 & ASF2_COLLDISABLE != 0, dead);
            assert_eq!(object.vy, 32767_i16.wrapping_add(2));
            assert_eq!(
                [object.worldx, object.worldy, object.worldz],
                [13, 20_i16.wrapping_add(32767), 35]
            );
            assert!(object.active);
            assert_eq!(game.objs.aldead, 0);
        }
    }
}

#[test]
fn relative_explosion_and_macro_facing_latch_are_independent() {
    let mut game = Game::new();
    let id = game.objs.alloc().unwrap();
    game.vars.pviewvelz = 65;
    for macro_latch in [false, true] {
        for relative in [false, true] {
            let object = &mut game.objs.aliens[id as usize];
            object.count = 0;
            object.count1 = 10;
            object.sflags2 = if macro_latch { ASF2_SMFLAG1 } else { 0 };
            object.sflags4 = if relative { ASF4_RELEXPLODE } else { 0 };
            object.worldz = 100;
            object.vz = 7;

            explode_end(&mut game, id);

            let object = game.objs.aliens[id as usize];
            assert_eq!(object.worldz, 107 + if relative { 65 } else { 0 });
            assert_eq!(object.sflags2 & ASF2_SMFLAG1 != 0, macro_latch);
            assert_eq!(game.objs.aldead, 0);
        }
    }
}
