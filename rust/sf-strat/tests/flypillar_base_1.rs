use sf_game::alien::{ASF3_NOHITAFFECT, ASF_SHADOW, ATZREMOVE};
use sf_game::game::Game;
use sf_game::obj::strat_init_obj_vars;
use sf_strat::enemies_ground::{base_1_istrat, base_1_strat, flypillar_istrat};

fn spawn(g: &mut Game, x: i16, y: i16, z: i16) -> u16 {
    let idx = g.objs.alloc().expect("object");
    strat_init_obj_vars(&mut g.objs.aliens[idx as usize]);
    let al = &mut g.objs.aliens[idx as usize];
    al.worldx = x;
    al.worldy = y;
    al.worldz = z;
    idx
}

#[test]
fn flypillar_uses_its_distinct_moving_istrat() {
    let mut g = Game::new();
    let _player = spawn(&mut g, 0, 0, 0);
    g.vars.internal_playpt = 0;
    g.vars.player_posx = 100;
    g.vars.rng = [7, 11, 13, 17];
    let pillar = spawn(&mut g, 0, -100, 5000);

    flypillar_istrat(&mut g, pillar);
    let al = g.objs.aliens[pillar as usize];
    assert_eq!((al.hp, al.ap), (12, 16));
    assert!(al.stratptr.is_some());
    assert!(al.collstratptr.is_some());
    assert!(al.expstratptr.is_some());
    assert_eq!(al.type_ & ATZREMOVE, 0, "noremove_behind while rising");
    assert_ne!(al.rotx, 0u8.wrapping_sub(64), "same-frame pitch chase ran");
    assert_eq!(
        al.worldy, -80,
        "rises 20 units outside the trigger distance"
    );
    assert_eq!(al.worldz, 5060, "al_vZ=60 is integrated");
    assert_ne!(al.sflags & ASF_SHADOW, 0);
    assert!(
        (-156..=355).contains(&al.worldx),
        "random X around player: {}",
        al.worldx
    );

    g.objs.aliens[pillar as usize].worldy = 0;
    let tick = g.objs.aliens[pillar as usize].stratptr.unwrap();
    g.call_strat(tick, pillar);
    let al = g.objs.aliens[pillar as usize];
    assert!(al.stratptr.is_none(), "ground-plane arrival ends the mover");
    assert_ne!(al.type_ & ATZREMOVE, 0);
    assert_eq!(al.sflags & ASF_SHADOW, 0);
}

#[test]
fn base_1_runs_the_autonomous_open_hold_close_cycle() {
    let mut g = Game::new();
    let door = spawn(&mut g, 50, 0, 1000);
    base_1_istrat(&mut g, door);
    let al = g.objs.aliens[door as usize];
    assert_eq!((al.hp, al.ap), (0xff, 8));
    assert_ne!(al.sflags3 & ASF3_NOHITAFFECT, 0);
    assert_eq!(al.animframe & 0x7f, 1, "init falls into the closing half");

    for _ in 0..20 {
        if g.objs.aliens[door as usize].sflags2 & 0x10 != 0 {
            break;
        }
        base_1_strat(&mut g, door);
    }
    assert_ne!(
        g.objs.aliens[door as usize].sflags2 & 0x10,
        0,
        "ten capped frames switch to the opening half",
    );
    assert_eq!(g.objs.aliens[door as usize].animframe & 0x7f, 7);

    for _ in 0..17 {
        base_1_strat(&mut g, door);
    }
    let al = g.objs.aliens[door as usize];
    assert_eq!(
        al.sflags2 & 0x10,
        0,
        "seven opening plus ten hold ticks restart close"
    );
    assert_eq!(
        al.animframe & 0x7f,
        1,
        "close begins in the transition frame"
    );
}
