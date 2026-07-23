//! Tick 167: true `zaco_8p` debris mesh (SHAPE_EXT 283) for szaco2.

use sf_game::alien::ASF3_REALOBJ;
use sf_game::game::Game;
use sf_strat::enemy_a::{explodedebris_istrat, strat_szaco2_init, SH_ZACO_8P};

fn spawn_player(g: &mut Game) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].sflags3 |= ASF3_REALOBJ;
    g.vars.internal_playpt = 0;
}

#[test]
fn szaco2_debrisshape_is_zaco_8p_ext() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = g.objs.alloc().expect("sz");
    g.objs.aliens[idx as usize].active = true;
    strat_szaco2_init(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].debrisshape, SH_ZACO_8P);
    assert_eq!(SH_ZACO_8P, 283);
}

#[test]
fn explodedebris_spawns_pieces_with_zaco_8p_shape() {
    let mut g = Game::new();
    spawn_player(&mut g);
    let idx = g.objs.alloc().expect("dead");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.active = true;
        al.worldx = 10;
        al.worldy = -20;
        al.worldz = 500;
        al.debrisshape = SH_ZACO_8P;
        al.hp = 0;
    }
    let before = g.objs.active_indices().len();
    explodedebris_istrat(&mut g, idx);
    let pieces: Vec<u16> = g
        .objs
        .active_indices()
        .into_iter()
        .filter(|&i| i != idx && i != 0 && g.objs.aliens[i as usize].shape == SH_ZACO_8P)
        .collect();
    assert_eq!(pieces.len(), 2, "two debris pieces with zaco_8p shape");
    assert!(g.objs.active_indices().len() > before);
    assert_eq!(
        g.objs.aliens[idx as usize].debrisshape, 0,
        "parent debrisshape cleared after spawn"
    );
}
