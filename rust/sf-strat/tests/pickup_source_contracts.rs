//! Live pickup/flash contracts from GASTRATS and PSTRATS, including exhaustion.
use std::{cell::RefCell, rc::Rc};

use sf_core::player_view::PlayerViewMode;
use sf_game::{
    alien::{AFONFIRE, ASF2_COLLDISABLE, ATZREMOVE},
    coldet::{PCBOX_WING_AP, PCBOX_WING_HP},
    game::Hooks,
    vars::PSF2_PLAYERHP0,
    Game,
};
use sf_strat::enemy_a::{flashplayer_istrat, item7a_istrat, SH_HELPBALL};

#[derive(Clone, Default)]
struct Sounds(Rc<RefCell<Vec<u8>>>);
impl Hooks for Sounds {
    fn play_se(&mut self, sound: u8) {
        self.0.borrow_mut().push(sound);
    }
}

fn scene() -> (Game, u16, u16, Sounds) {
    let sounds = Sounds::default();
    let mut game = Game::with_hooks(Box::new(sounds.clone()));
    // Exposed player is deliberately not slot zero or internal_playpt.
    let decoy = game.objs.alloc().unwrap();
    game.objs.aliens[decoy as usize].worldz = 10_000;
    let player = game.objs.alloc().unwrap();
    game.vars.player_object = player as i16;
    game.vars.internal_playpt = decoy as i16;
    game.vars.minpmove_y = -50;
    game.vars.gameframe = 1;
    let ship = &mut game.objs.aliens[player as usize];
    [ship.worldx, ship.worldy, ship.worldz] = [32760, 0, 32760];
    [ship.rotx, ship.roty, ship.rotz] = [5, 17, 29];
    let pickup = game.objs.alloc().unwrap();
    let item = &mut game.objs.aliens[pickup as usize];
    [item.worldx, item.worldy, item.worldz] = [-32760, 0, -32760];
    item.sbyte1 = 1;
    item.hp = 1;
    item.shape = 123;
    item.sflags = 0xA4;
    item.sflags2 = 0xB0;
    item.colframe = 0x82;
    (game, player, pickup, sounds)
}

#[test]
fn failed_helper_allocation_still_flashes_but_preserves_wings_flags_and_sound() {
    for cockpit in [false, true] {
        let (mut game, player, pickup, sounds) = scene();
        let left = game.objs.alloc().unwrap();
        let right = game.objs.alloc().unwrap();
        game.coldet.pcbox.lwing = Some(left);
        game.coldet.pcbox.rwing = Some(right);
        game.objs.aliens[left as usize].hp = 2;
        game.objs.aliens[right as usize].hp = 1;
        game.vars.pshipflags = 0xFF;
        if cockpit {
            game.vars.player_view_mode = PlayerViewMode::Cockpit;
        }
        while game.objs.alloc().is_some() {}
        let before_left = game.objs.aliens[left as usize];
        let before_right = game.objs.aliens[right as usize];
        item7a_istrat(&mut game, pickup);
        assert_eq!(game.objs.aliens[left as usize], before_left);
        assert_eq!(game.objs.aliens[right as usize], before_right);
        assert_eq!(game.vars.pshipflags, 0xFF);
        assert!(sounds.0.borrow().is_empty());
        let item = game.objs.aliens[pickup as usize];
        assert_eq!(item.sflags, 0xA4);
        assert_eq!(item.sflags2, 0xB0 | ASF2_COLLDISABLE);
        if cockpit {
            assert_eq!(game.objs.aldead, 1);
            assert_eq!(item.count, 0);
        } else {
            assert_eq!(item.count, 19);
            assert_eq!(item.worldx, game.objs.aliens[player as usize].worldx);
            assert_eq!(item.worldz, game.objs.aliens[player as usize].worldz);
            assert_eq!(item.colframe, 0x83);
            assert_eq!(item.shape, 354); // both broken-wing flags survive
        }
    }
}

#[test]
fn successful_pickup_restores_wing_handlers_and_installs_one_deferred_helper() {
    let (mut game, _, pickup, sounds) = scene();
    let ids = sf_strat::player::install(&mut game);
    let left = game.objs.alloc().unwrap();
    let right = game.objs.alloc().unwrap();
    game.coldet.pcbox.lwing = Some(left);
    game.coldet.pcbox.rwing = Some(right);
    game.vars.pshipflags = 0xFF;
    for wing in [left, right] {
        let al = &mut game.objs.aliens[wing as usize];
        al.hp = 0;
        al.ap = 0;
        al.type_ = 0xFF;
        al.sflags = 0xD2;
        al.sflags2 = 0xB0;
        al.endcollstratptr = Some(ids.player_coll);
        al.stratstate = 77;
    }
    item7a_istrat(&mut game, pickup);
    assert_eq!(*sounds.0.borrow(), [0x10]);
    assert_eq!(game.vars.pshipflags, 0xFF);
    for wing in [left, right] {
        let al = &game.objs.aliens[wing as usize];
        assert_eq!((al.hp, al.ap), (PCBOX_WING_HP, PCBOX_WING_AP));
        assert_eq!(al.type_, 0xFF & !ATZREMOVE);
        assert_eq!(al.sflags, 0xD2);
        assert_eq!(al.sflags2, 0xB0 | ASF2_COLLDISABLE);
        assert_eq!(al.stratptr, Some(ids.pcbox_wing));
        assert_eq!(al.collstratptr, Some(ids.pcbox_coll));
        assert_eq!(al.expstratptr, Some(ids.pcbox_coll));
        assert_eq!(al.endcollstratptr, Some(ids.player_coll));
    }
    let actors = game.objs.active_indices();
    let item_index = actors.iter().position(|&id| id == pickup).unwrap();
    let ball = &game.objs.aliens[actors[item_index + 1] as usize];
    assert_eq!(ball.shape, SH_HELPBALL);
    assert_eq!([ball.worldx, ball.worldy, ball.worldz], [0; 3]);
    assert_eq!(ball.sbyte3, 0, "helper has not taken its own first visit");
    assert_eq!(game.objs.aliens[pickup as usize].count, 19);
}

#[test]
fn flash_marks_removal_after_following_but_finishes_animation_and_lifetime() {
    let (mut game, player, pickup, _) = scene();
    game.vars.pshipflags2 |= PSF2_PLAYERHP0;
    flashplayer_istrat(&mut game, pickup);
    let ship = game.objs.aliens[player as usize];
    let item = game.objs.aliens[pickup as usize];
    assert_eq!(
        [item.worldx, item.worldy, item.worldz],
        [ship.worldx, ship.worldy, ship.worldz]
    );
    assert_eq!(
        [item.rotx, item.roty, item.rotz],
        [ship.rotx, ship.roty, ship.rotz]
    );
    assert_eq!(item.count, 19, "death marks removal without returning");
    assert_eq!(item.colframe, 0x83);
    assert_eq!(item.shape, 351);
    assert_eq!(game.objs.aldead, 1);
}

#[test]
fn repair_catch_defers_flash_entry_until_its_next_strategy_visit() {
    let mut game = Game::new();
    game.objs.alloc().unwrap();
    let pod = game.objs.alloc().unwrap();
    sf_strat::enemy_a::ripair_istrat(&mut game, pod);
    let item = &mut game.objs.aliens[pod as usize];
    [item.worldx, item.worldy, item.worldz] = [0, 0, -30];
    item.sbyte1 = 1;
    item.count = 91;
    item.colframe = 0x82;
    item.shape = 401;
    sf_strat::enemy_a::ripair_strat(&mut game, pod);
    assert_eq!(game.objs.aliens[pod as usize].count, 91);
    assert_eq!(game.objs.aliens[pod as usize].shape, 401);
    assert_eq!(game.objs.aliens[pod as usize].colframe, 0x82);
    let installed = game.objs.aliens[pod as usize].stratptr.unwrap();
    game.call_strat(installed, pod);
    assert_eq!(game.objs.aliens[pod as usize].count, 19);
    assert_eq!(game.objs.aliens[pod as usize].shape, 0);
}

#[test]
fn death_mark_releases_fire_before_full_pool_collection_and_still_installs_the_helper() {
    let (mut game, _, pickup, sounds) = scene();
    let fire = game.objs.alloc().unwrap();
    game.objs.aliens[pickup as usize].flags = AFONFIRE | 0x10;
    game.objs.aliens[pickup as usize].fireobjptr = fire + 1;
    game.vars.pshipflags2 |= PSF2_PLAYERHP0;
    while game.objs.alloc().is_some() {}
    item7a_istrat(&mut game, pickup);
    assert_eq!(game.objs.aldead, 2, "pickup and flash both mark retirement");
    assert_eq!(game.objs.aliens[pickup as usize].flags, 0x10);
    assert_eq!(game.objs.aliens[pickup as usize].fireobjptr, 0);
    assert_eq!(game.objs.aliens[fire as usize].shape, SH_HELPBALL);
    assert_eq!(game.objs.aliens[pickup as usize].count, 19);
    assert_eq!(*sounds.0.borrow(), [0x10]);
}

#[test]
fn removal_preserves_an_unlinked_fire_flag() {
    let (mut game, _, pickup, _) = scene();
    game.objs.aliens[pickup as usize].flags = AFONFIRE | 0x10;
    game.vars.player_view_mode = PlayerViewMode::Cockpit;
    flashplayer_istrat(&mut game, pickup);
    assert_eq!(game.objs.aliens[pickup as usize].flags, AFONFIRE | 0x10);
    assert_eq!(game.objs.aldead, 1);
}

#[test]
fn production_pass_consumes_the_pickup_once_then_retires_its_flash_after_twenty_visits() {
    let (mut game, player, pickup, sounds) = scene();
    let init = game.world.register_strategy(item7a_istrat);
    game.objs.aliens[pickup as usize].stratptr = Some(init);
    game.run_strategies();
    assert_eq!(game.objs.aliens[pickup as usize].count, 19);
    let balls: Vec<_> = game
        .objs
        .active_indices()
        .into_iter()
        .filter(|&id| game.objs.aliens[id as usize].shape == SH_HELPBALL)
        .collect();
    assert_eq!(balls.len(), 1);
    assert_eq!(game.objs.aliens[balls[0] as usize].sbyte3, 30);
    assert_eq!(
        game.objs.aliens[balls[0] as usize].worldz,
        game.objs.aliens[player as usize].worldz.wrapping_add(60)
    );
    for expected in (1..=18).rev() {
        game.run_strategies();
        assert!(game.objs.aliens[pickup as usize].active);
        assert_eq!(game.objs.aliens[pickup as usize].count, expected);
    }
    game.run_strategies();
    assert!(!game.objs.aliens[pickup as usize].active);
    assert!(game.objs.aliens[balls[0] as usize].active);
    assert_eq!(*sounds.0.borrow(), [0x10]);
}
