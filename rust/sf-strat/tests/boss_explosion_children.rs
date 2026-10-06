//! The boss-circle producer must reach the shipping scheduler and particle draw.
use sf_core::{particles::ParticleField, screen_fill_circle::ScreenFillCircleCenter};
use sf_game::{
    alien::{ASF4_PLAYEROBJ, ASF_PARTOBJ},
    draw::{build_list, CullView},
    Game,
};
use sf_strat::enemy_a::{circdelayexplode_init, ASF2_SFLAG1};

#[test]
fn circle_children_run_once_in_birth_pass_and_emit_through_the_production_draw_list() {
    for free in 0..=2 {
        let mut game = Game::new();
        let player = game.objs.alloc().unwrap();
        game.objs.aliens[player as usize].sflags4 = ASF4_PLAYEROBJ;
        game.vars.internal_playpt = player as i16;
        game.vars.player_object = player as i16;
        game.vars.pviewvelz = 1;
        let parent = game.objs.alloc().unwrap();
        let init = game.world.register_strategy(circdelayexplode_init);
        let object = &mut game.objs.aliens[parent as usize];
        object.stratptr = Some(init);
        object.worldz = 1024;
        object.sflags2 = ASF2_SFLAG1;
        while game.objs.active_indices().len() < game.objs.aliens.len() - free {
            game.objs.alloc().unwrap();
        }
        let first_free = game.objs.free_head;
        game.run_strategies();
        assert!(!game.objs.aliens[parent as usize].active);
        assert_eq!(game.objs.aliens[parent as usize].count, 255);
        assert_eq!(game.objs.aliens[parent as usize].worldz, 1025);
        if let Some(anchor) = first_free {
            assert_eq!(
                game.vars.screen_fill_circle.center,
                ScreenFillCircleCenter::Object(anchor + 1)
            );
            assert_eq!(
                game.objs.aliens[anchor as usize].worldz, 1025,
                "anchor visits once in birth pass"
            );
        } else {
            assert_eq!(
                game.vars.screen_fill_circle.center,
                ScreenFillCircleCenter::Screen
            );
        }
        let emitter = game
            .objs
            .aliens
            .iter()
            .position(|al| al.active && al.sflags & ASF_PARTOBJ != 0);
        assert_eq!(
            emitter.is_some(),
            free == 2,
            "parent is not recycled until after both allocation attempts"
        );
        let mut field = ParticleField::default();
        for visit in 0..=110 {
            if visit > 0 {
                game.run_strategies();
            }
            if let Some(emitter) = emitter {
                let object = &game.objs.aliens[emitter];
                assert_eq!(object.active, visit < 110);
                assert_eq!(object.count, visit);
                assert_eq!(object.worldz, 1024 + i16::from(visit.min(109)));
                if visit == 0 {
                    assert_eq!([object.sbyte3, object.sbyte1, object.sbyte2], [4, 255, 100]);
                } else {
                    assert_eq!([object.sbyte3, object.sbyte1, object.sbyte2], [0; 3]);
                }
            }
            let mut entries = Vec::new();
            build_list(
                &mut game.objs,
                0,
                game.vars.gameframe,
                CullView::default(),
                0,
                &|_| None,
                &mut entries,
            );
            let frame = field.capture_scene(true, &entries, [0; 3], [0; 3]);
            assert_eq!(
                frame.work.particles_allocated,
                if visit == 0 && free == 2 { 255 } else { 0 }
            );
            if visit == 0 && free == 2 {
                assert_eq!(frame.work.particles_visited, 255);
                assert!(frame.draws.iter().any(|draw| !draw.primitives.is_empty()));
            }
        }
        assert!(field.particles().iter().all(|particle| particle.life == 0));
    }
}

#[test]
fn outward_boss_explosion_defers_particle_init_until_its_same_pass_visit() {
    let mut game = Game::new();
    let player = game.objs.alloc().unwrap();
    game.objs.aliens[player as usize].sflags4 = ASF4_PLAYEROBJ;
    game.vars.internal_playpt = player as i16;
    game.vars.player_object = player as i16;
    let boss = game.objs.alloc().unwrap();
    let init = game
        .world
        .register_strategy(sf_strat::enemy_b::bossbigoutexplode_istrat);
    game.objs.aliens[boss as usize].stratptr = Some(init);
    game.objs.aliens[boss as usize].worldz = 1024;
    while game.objs.active_indices().len() < game.objs.aliens.len() - 2 {
        game.objs.alloc().unwrap();
    }
    let mut field = ParticleField::default();
    for scene in 0..2 {
        game.run_strategies();
        let mut entries = Vec::new();
        build_list(
            &mut game.objs,
            0,
            game.vars.gameframe,
            CullView::default(),
            0,
            &|_| None,
            &mut entries,
        );
        let frame = field.capture_scene(true, &entries, [0; 3], [0; 3]);
        assert_eq!(
            frame.work.particles_allocated,
            if scene == 0 { 255 } else { 0 }
        );
        assert_eq!(frame.work.particles_visited, 255);
        let emitter = game
            .objs
            .aliens
            .iter()
            .find(|al| al.active && al.sflags & ASF_PARTOBJ != 0)
            .unwrap();
        assert_eq!(emitter.count, scene);
    }
}
