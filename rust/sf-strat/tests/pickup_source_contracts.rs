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
use sf_strat::enemies_ground::{item3_istrat, winglazermandie_istrat, wldie_istrat};
use sf_strat::enemy_a::{
    bomwingdie_istrat, flashplayer_istrat, item4_istrat, item4_strat, item7a_istrat, ripair_istrat,
    ripair_strat, strat_item5_init, strat_item7_init, PSF_BRKLWING, PSF_BRKRWING, PSF_LWINGCOLL,
    PSF_RWINGCOLL, SH_HELPBALL,
};

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

#[test]
fn repair_pickup_keeps_moving_after_failed_allocation_and_collects_when_a_slot_is_freed() {
    let (mut game, player, pickup, sounds) = scene();
    game.objs.aliens[player as usize].worldz = 0;
    game.objs.aliens[pickup as usize].worldz = -20;
    let before = game.objs.aliens[pickup as usize];
    item4_istrat(&mut game, pickup);
    let initialized = game.objs.aliens[pickup as usize];
    assert_eq!(initialized.worldz, before.worldz, "entry does not move");
    assert_eq!(initialized.sflags, before.sflags);
    assert_eq!(initialized.sflags2, before.sflags2 | ASF2_COLLDISABLE);
    let slot = game.objs.alloc().unwrap();
    while game.objs.alloc().is_some() {}
    item4_strat(&mut game, pickup);
    assert_eq!(
        game.objs.aldead, 0,
        "failed allocation does not consume pickup"
    );
    assert_eq!(game.objs.aliens[pickup as usize].worldz, 0);
    assert_eq!(
        game.objs.aliens[pickup as usize].rotx,
        before.rotx.wrapping_add(4)
    );
    assert!(sounds.0.borrow().is_empty());
    game.objs.free(slot);
    item4_strat(&mut game, pickup);
    assert_eq!(game.objs.aldead, 1);
    assert_eq!(game.objs.aliens[slot as usize].shape, 401);
    assert_eq!(game.objs.aliens[slot as usize].sbyte1, 0);
    assert!(sounds.0.borrow().is_empty(), "new ship has not visited yet");
}

#[test]
fn repair_catch_restores_wing_actors_then_clears_only_the_four_wing_flags() {
    let (mut game, player, pod, sounds) = scene();
    let ids = sf_strat::player::install(&mut game);
    let left = game.objs.alloc().unwrap();
    let right = game.objs.alloc().unwrap();
    game.coldet.pcbox.lwing = Some(left);
    game.coldet.pcbox.rwing = Some(right);
    for wing in [left, right] {
        let al = &mut game.objs.aliens[wing as usize];
        al.hp = 0;
        al.ap = 1;
        al.type_ = 0xFF;
        al.sflags = 0xA4;
        al.sflags2 = 0xB0;
        al.endcollstratptr = Some(ids.player_coll);
    }
    let pl = game.objs.aliens[player as usize];
    [
        game.vars.player_posx,
        game.vars.player_posy,
        game.vars.player_posz,
    ] = [pl.worldx, pl.worldy, pl.worldz];
    ripair_istrat(&mut game, pod);
    game.vars.pshipflags = 0xFF;
    // The repair routine does not return early on player-death flags.
    game.vars.pshipflags2 = PSF2_PLAYERHP0;
    let al = &mut game.objs.aliens[pod as usize];
    [al.worldx, al.worldy, al.worldz] = [pl.worldx, pl.worldy, pl.worldz.wrapping_sub(30)];
    al.sbyte1 = 1;
    let count = al.count;
    ripair_strat(&mut game, pod);
    for wing in [left, right] {
        let al = &game.objs.aliens[wing as usize];
        assert_eq!((al.hp, al.ap), (PCBOX_WING_HP, PCBOX_WING_AP));
        assert_eq!(al.type_, 0xFF & !ATZREMOVE);
        assert_eq!(al.sflags, 0xA4);
        assert_eq!(al.sflags2, 0xB0 | ASF2_COLLDISABLE);
        assert_eq!(al.stratptr, Some(ids.pcbox_wing));
        assert_eq!(al.collstratptr, Some(ids.pcbox_coll));
        assert_eq!(al.expstratptr, Some(ids.pcbox_coll));
        assert_eq!(al.endcollstratptr, Some(ids.player_coll));
    }
    assert_eq!(
        game.vars.pshipflags,
        !(PSF_BRKLWING | PSF_BRKRWING | PSF_LWINGCOLL | PSF_RWINGCOLL)
    );
    assert_eq!(game.objs.aliens[pod as usize].count, count);
    assert_eq!(*sounds.0.borrow(), [0x8B, 0x17]);
}

#[test]
fn intact_laser_pickup_restores_wings_without_clearing_collision_flags() {
    let (mut game, _, pickup, sounds) = scene();
    let left = game.objs.alloc().unwrap();
    let right = game.objs.alloc().unwrap();
    game.coldet.pcbox.lwing = Some(left);
    game.coldet.pcbox.rwing = Some(right);
    game.objs.aliens[left as usize].hp = 0;
    game.objs.aliens[right as usize].hp = 0;
    let flags = 0xFF & !(PSF_BRKLWING | PSF_BRKRWING);
    game.vars.pshipflags = flags;
    game.vars.shared.player_score = u16::MAX - 49;
    strat_item7_init(&mut game, pickup);
    assert_eq!(game.vars.pshipflags, flags);
    assert_eq!(game.objs.aliens[left as usize].hp, PCBOX_WING_HP);
    assert_eq!(game.objs.aliens[right as usize].hp, PCBOX_WING_HP);
    assert_eq!(game.vars.shared.player_score, u16::MAX - 49);
    assert_eq!(*sounds.0.borrow(), [0x15]);
}

#[test]
fn production_pass_initializes_repair_ship_once_after_removing_its_pickup() {
    let (mut game, player, pickup, sounds) = scene();
    let pl = game.objs.aliens[player as usize];
    [
        game.vars.player_posx,
        game.vars.player_posy,
        game.vars.player_posz,
    ] = [pl.worldx, pl.worldy, pl.worldz];
    let init = game.world.register_strategy(item4_istrat);
    game.objs.aliens[pickup as usize].stratptr = Some(init);
    game.run_strategies();
    assert!(game.objs.aliens[pickup as usize].active);
    assert!(sounds.0.borrow().is_empty());
    game.run_strategies();
    assert!(!game.objs.aliens[pickup as usize].active);
    let ships: Vec<_> = game
        .objs
        .active_indices()
        .into_iter()
        .filter(|&id| game.objs.aliens[id as usize].shape == 401)
        .collect();
    assert_eq!(ships.len(), 1);
    let ship = game.objs.aliens[ships[0] as usize];
    // The frame owner publishes internal-player coordinates separately from
    // the exposed actor used by pickup collision, just as the source does.
    assert_eq!(
        [ship.worldx, ship.worldy, ship.worldz],
        [
            game.vars.player_posx.wrapping_add(500),
            game.vars.player_posy,
            game.vars.player_posz.wrapping_sub(200)
        ]
    );
    assert_eq!(ship.sbyte1, 30, "entry does not enter the motion body");
    assert_eq!(*sounds.0.borrow(), [0x8B]);
    game.run_strategies();
    assert_eq!(game.objs.aliens[ships[0] as usize].sbyte1, 29);
    assert_eq!(*sounds.0.borrow(), [0x8B]);
}

#[test]
fn repair_pickup_death_marker_releases_fire_then_still_allocates_and_marks_again() {
    let (mut game, _, pickup, sounds) = scene();
    let fire = game.objs.alloc().unwrap();
    game.objs.aliens[pickup as usize].flags = AFONFIRE | 0x10;
    game.objs.aliens[pickup as usize].fireobjptr = fire + 1;
    game.vars.pshipflags2 = PSF2_PLAYERHP0;
    game.objs.aldead = 254;
    while game.objs.alloc().is_some() {}
    item4_strat(&mut game, pickup);
    assert_eq!(game.objs.aldead, 0, "both source markers wrap the byte");
    assert_eq!(game.objs.aliens[pickup as usize].flags, 0x10);
    assert_eq!(game.objs.aliens[pickup as usize].fireobjptr, 0);
    assert_eq!(game.objs.aliens[fire as usize].shape, 401);
    assert_eq!(game.objs.aliens[fire as usize].sbyte1, 0);
    assert!(sounds.0.borrow().is_empty());
}

#[test]
fn special_pickup_uses_the_signed_word_cap_and_consumes_even_when_full_or_dead() {
    for (before, after) in [
        (0, 1),
        (4, 5),
        (5, 5),
        (6, 6),
        (32772, 32772),
        (32773, 32774),
        (65535, 0),
    ] {
        for dead in [false, true] {
            for cockpit in [false, true] {
                let (mut game, _, pickup, sounds) = scene();
                game.vars.strategy.special_weapon_count = before;
                game.vars.shared.special_flash = 201;
                game.vars.shared.player_score = 47;
                game.objs.aliens[pickup as usize].count = 91;
                if dead {
                    game.vars.pshipflags2 = PSF2_PLAYERHP0;
                }
                if cockpit {
                    game.vars.player_view_mode = PlayerViewMode::Cockpit;
                }
                strat_item5_init(&mut game, pickup);
                assert_eq!(game.vars.strategy.special_weapon_count, after);
                assert_eq!(
                    game.vars.shared.special_flash,
                    if before == after { 201 } else { 30 }
                );
                assert_eq!(game.vars.shared.player_score, 47);
                assert_eq!(
                    *sounds.0.borrow(),
                    if before == after { vec![] } else { vec![0x18] }
                );
                let item = game.objs.aliens[pickup as usize];
                assert_eq!(item.sflags, 0xA4);
                assert_eq!(item.sflags2, 0xB0 | ASF2_COLLDISABLE);
                assert_eq!(item.count, if cockpit { 91 } else { 19 });
                assert_eq!(game.objs.aldead, u8::from(dead) + u8::from(dead || cockpit));
            }
        }
    }
}

#[test]
fn special_drop_only_awards_inventory_when_its_own_entry_visits_the_player() {
    let (mut game, player, bomber, sounds) = scene();
    let pl = game.objs.aliens[player as usize];
    let al = &mut game.objs.aliens[bomber as usize];
    [al.worldx, al.worldy, al.worldz] = [pl.worldx, pl.worldy, pl.worldz];
    al.sflags = 0;
    al.sflags2 = 0;
    game.vars.strategy.special_weapon_count = 4;
    bomwingdie_istrat(&mut game, bomber);
    assert_eq!(game.vars.strategy.special_weapon_count, 4);
    assert!(sounds.0.borrow().is_empty());
    let active = game.objs.active_indices();
    let child = active[active.iter().position(|&id| id == bomber).unwrap() + 1];
    let drop = game.objs.aliens[child as usize];
    assert_eq!(drop.shape, 158);
    assert_eq!(
        [drop.worldx, drop.worldy, drop.worldz],
        [pl.worldx, pl.worldy.wrapping_sub(20), pl.worldz]
    );
    assert_eq!(drop.sflags2 & ASF2_COLLDISABLE, 0);
    game.objs.aldead = 0;
    game.call_strat(drop.stratptr.unwrap(), child);
    assert_eq!(game.vars.strategy.special_weapon_count, 5);
    assert_eq!(game.objs.aliens[child as usize].count, 19);
    assert_eq!(*sounds.0.borrow(), [0x18]);
}

#[test]
fn production_pass_enters_the_special_drop_once_before_its_next_flash_visit() {
    let (mut game, player, bomber, sounds) = scene();
    let pl = game.objs.aliens[player as usize];
    let init = game.world.register_strategy(bomwingdie_istrat);
    let al = &mut game.objs.aliens[bomber as usize];
    [al.worldx, al.worldy, al.worldz] = [pl.worldx, pl.worldy, pl.worldz];
    al.sflags = 0;
    al.sflags2 = 0;
    al.stratptr = Some(init);
    game.vars.strategy.special_weapon_count = 4;
    game.run_strategies();
    assert!(!game.objs.aliens[bomber as usize].active);
    let flash = game
        .objs
        .active_indices()
        .into_iter()
        .find(|&id| game.objs.aliens[id as usize].count == 19)
        .unwrap();
    assert_eq!(game.vars.strategy.special_weapon_count, 5);
    assert_eq!(*sounds.0.borrow(), [0x18]);
    game.run_strategies();
    assert_eq!(game.objs.aliens[flash as usize].count, 18);
    assert_eq!(game.vars.strategy.special_weapon_count, 5);
    assert_eq!(*sounds.0.borrow(), [0x18]);
}

#[test]
fn body_pickup_heals_live_collision_body_once_in_the_production_pass() {
    let (mut game, _, pickup, sounds) = scene();
    let body = game.objs.alloc().unwrap();
    game.coldet.pcbox.body = Some(body);
    game.objs.aliens[body as usize].hp = 37;
    game.objs.aliens[0].hp = 17;
    game.vars.strategy.player_collision_objects[0] = 0;
    let entry = game.world.register_strategy(item3_istrat);
    game.objs.aliens[pickup as usize].stratptr = Some(entry);
    game.run_strategies();
    assert_eq!(game.objs.aliens[body as usize].hp, 40);
    assert_eq!(game.objs.aliens[0].hp, 17);
    assert_eq!(game.objs.aliens[pickup as usize].count, 19);
    assert_eq!(*sounds.0.borrow(), [0x10]);
    game.objs.aliens[body as usize].hp = 31;
    game.run_strategies();
    assert_eq!(game.objs.aliens[body as usize].hp, 31);
    assert_eq!(game.objs.aliens[pickup as usize].count, 18);
    assert_eq!(*sounds.0.borrow(), [0x10]);
}

#[test]
fn dead_body_pickup_releases_fire_and_still_heals_before_inline_flash_removal() {
    let (mut game, _, pickup, sounds) = scene();
    let body = game.objs.alloc().unwrap();
    game.coldet.pcbox.body = Some(body);
    game.objs.aliens[body as usize].hp = 9;
    let fire = game.objs.alloc().unwrap();
    game.objs.aliens[pickup as usize].flags = AFONFIRE | 0x10;
    game.objs.aliens[pickup as usize].fireobjptr = fire + 1;
    game.vars.pshipflags2 = PSF2_PLAYERHP0;
    game.objs.aldead = 254;
    item3_istrat(&mut game, pickup);
    assert_eq!(game.objs.aliens[body as usize].hp, 14);
    assert!(!game.objs.aliens[fire as usize].active);
    assert_eq!(game.objs.aliens[pickup as usize].fireobjptr, 0);
    assert_eq!(game.objs.aliens[pickup as usize].flags, 0x10);
    assert_eq!(game.objs.aldead, 0);
    assert_eq!(*sounds.0.borrow(), [0x10]);
}

#[test]
fn laser_drops_inherit_position_before_collection_and_visit_once_in_production() {
    for entry in [wldie_istrat as fn(&mut Game, u16), winglazermandie_istrat] {
        for production in [false, true] {
            let (mut game, player, parent, sounds) = scene();
            let ship = game.objs.aliens[player as usize];
            let al = &mut game.objs.aliens[parent as usize];
            [al.worldx, al.worldy, al.worldz] = [ship.worldx, ship.worldy, ship.worldz];
            al.sflags = 0;
            al.sflags2 = 0;
            let child;
            if production {
                let strategy = game.world.register_strategy(entry);
                game.objs.aliens[parent as usize].stratptr = Some(strategy);
                game.run_strategies();
                assert!(!game.objs.aliens[parent as usize].active);
                child = game
                    .objs
                    .active_indices()
                    .into_iter()
                    .find(|&id| game.objs.aliens[id as usize].count == 19)
                    .unwrap();
            } else {
                entry(&mut game, parent);
                assert_eq!(game.vars.pshipflags2, 0);
                assert!(sounds.0.borrow().is_empty());
                let active = game.objs.active_indices();
                child = active[active.iter().position(|&id| id == parent).unwrap() + 1];
                let drop = game.objs.aliens[child as usize];
                assert_eq!(drop.shape, 160);
                assert_eq!(
                    [drop.worldx, drop.worldy, drop.worldz],
                    [ship.worldx, ship.worldy, ship.worldz]
                );
                assert_eq!(drop.sflags2 & ASF2_COLLDISABLE, 0);
                game.objs.aldead = 0;
                game.call_strat(drop.stratptr.unwrap(), child);
            }
            assert_ne!(game.vars.pshipflags2 & sf_strat::enemy_a::PSF2_DOUBLASER, 0);
            assert_eq!(game.objs.aliens[child as usize].count, 19);
            assert_eq!(*sounds.0.borrow(), [0x15]);
            let next = game.objs.aliens[child as usize].stratptr.unwrap();
            game.call_strat(next, child);
            assert_eq!(game.objs.aliens[child as usize].count, 18);
            assert_eq!(*sounds.0.borrow(), [0x15]);
        }
    }
}
