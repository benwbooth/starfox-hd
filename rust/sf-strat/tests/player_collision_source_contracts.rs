//! Collision branches recovered from PSTRATS, independent of replay outcomes.

use sf_game::{alien::ASF_COLLIDE, game::Hooks, vars::FRAMESPERAP, Game};
use sf_strat::{
    common::{sv, StratRam},
    player::{pcbox_attach, pcollw_istrat, pcollw_strat, pcolrw_istrat, pcolrw_strat},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
struct Sounds(Rc<RefCell<Vec<u8>>>);
impl Hooks for Sounds {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

fn scene(right: bool) -> (Game, u16, u16, u16, Sounds) {
    let sounds = Sounds::default();
    let mut game = Game::with_hooks(Box::new(sounds.clone()));
    let player = game.objs.alloc().unwrap();
    assert!(pcbox_attach(&mut game, player));
    game.vars.player_object = player as i16;
    let wing = if right {
        game.coldet.pcbox.rwing
    } else {
        game.coldet.pcbox.lwing
    }
    .unwrap();
    let body = game.coldet.pcbox.body.unwrap();
    (game, player, wing, body, sounds)
}

#[test]
fn broken_wing_wall_damage_uses_body_cooldown_and_not_wire_scaling() {
    for right in [false, true] {
        for wire in [false, true] {
            for cooldown in [0u8, 1, 2, 255] {
                let (mut game, _, wing, body, _) = scene(right);
                game.vars.pshipflags = if right { 16 } else { 8 };
                game.vars.pshipflags2 = if wire { 2 } else { 0 };
                game.objs.aliens[body as usize].hp = 40;
                game.objs.aliens[body as usize].collcount = cooldown;
                game.objs.aliens[wing as usize].collcount = 99;
                game.objs.aliens[wing as usize].collobjptr = 0;
                if right {
                    pcolrw_strat(&mut game, wing);
                } else {
                    pcollw_strat(&mut game, wing);
                }
                let actual = &game.objs.aliens[body as usize];
                assert_eq!(actual.hp, if cooldown == 1 { 36 } else { 40 });
                assert_eq!(
                    actual.collcount,
                    if cooldown == 1 {
                        FRAMESPERAP
                    } else {
                        cooldown.wrapping_sub(1)
                    }
                );
                assert_ne!(actual.sflags & ASF_COLLIDE, 0);
                assert_eq!(actual.collobjptr, 0);
                assert_eq!(game.objs.aliens[wing as usize].collcount, 99);
            }
        }
    }
}

#[test]
fn wing_wall_entry_has_no_object_impact_recoil_or_wire_impact_sound() {
    for right in [false, true] {
        for wire in [false, true] {
            for roll in [0, 128] {
                let (mut game, player, wing, _, sounds) = scene(right);
                game.objs.aliens[player as usize].worldx = 100;
                game.objs.aliens[player as usize].rotz = roll;
                game.objs.aliens[wing as usize].collobjptr = 0;
                game.vars.set_sv_i16(sv::PLROTX, 1234);
                game.vars.set_sv_i16(sv::PLAYER_ZSHAKE, 5678);
                game.vars.pshipflags2 = if wire { 2 } else { 0 };
                while game.objs.alloc().is_some() {}
                if right {
                    pcolrw_istrat(&mut game, wing);
                } else {
                    pcollw_istrat(&mut game, wing);
                }
                assert_eq!(game.objs.aliens[player as usize].worldx, 100);
                assert_eq!(game.vars.sv_i16(sv::PLROTX), 1234);
                assert_eq!(game.vars.sv_i16(sv::PLAYER_ZSHAKE), 5678);
                assert_eq!(
                    *sounds.0.borrow(),
                    if wire {
                        vec![0x14]
                    } else {
                        vec![if right { 8 } else { 7 }]
                    }
                );
            }
        }
    }
}

#[test]
fn both_wing_states_share_effect_copy_and_missing_effect_suppresses_sparks() {
    for right in [false, true] {
        for broken in [false, true] {
            for effect_present in [false, true] {
                let (mut game, player, wing, _, _) = scene(right);
                if broken {
                    game.vars.pshipflags = if right { 16 } else { 8 };
                }
                let ship = &mut game.objs.aliens[player as usize];
                [ship.worldx, ship.worldy, ship.worldz] = [100, 200, 300];
                let effect = effect_present.then(|| game.objs.alloc().unwrap());
                game.objs.aliens[wing as usize].sword1 = effect.unwrap_or(0) as i16;
                let before = game.objs.aliens.iter().filter(|al| al.active).count();
                if right {
                    pcolrw_strat(&mut game, wing);
                } else {
                    pcollw_strat(&mut game, wing);
                }
                assert_eq!(
                    game.objs.aliens.iter().filter(|al| al.active).count(),
                    before + usize::from(effect_present)
                );
                if let Some(effect) = effect {
                    let wing = &game.objs.aliens[wing as usize];
                    let effect = &game.objs.aliens[effect as usize];
                    assert_eq!(
                        [effect.worldx, effect.worldy, effect.worldz],
                        [wing.worldx, wing.worldy, wing.worldz]
                    );
                }
            }
        }
    }
}

#[test]
fn damage_scaling_and_clamping_use_signed_wrapping_byte_operations() {
    let mut game = Game::new();
    let victim = game.objs.alloc().unwrap();
    for (ap, scale, hp, expected) in [
        (255, 1, 20, 21),
        (128, 2, 20, 52),
        (255, 0, 127, 0),
        (200, 0, 20, 76),
    ] {
        game.objs.aliens[victim as usize].hp = hp;
        game.objs.aliens[victim as usize].collcount = 1;
        game.coldet_apply_damage(victim, ap, scale);
        assert_eq!(game.objs.aliens[victim as usize].hp, expected);
    }
}
