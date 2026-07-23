//! `s_goto_WP` / path face+GOTOPOS aim via `sf_core::aim_angle`.

use sf_core::aim_angle::{xanglexabs, xanglexy, yanglexy, yanglexy_nega};
use sf_game::alien::Alien;
use sf_game::game::Game;
use sf_strat::enemies_ground::{saucer1_istrat, saucer1_strat};

fn alloc(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize] = Alien::default();
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn obj2wp_angle_targets_match_rom_helpers() {
    // +X WP: Yanglexy=64 → nega=192; flat elev → 0.
    assert_eq!(yanglexy(1000, 0), 64);
    assert_eq!(yanglexy_nega(1000, 0), 192);
    assert_eq!(xanglexabs(0, 1000, 0), 0);
    // Elevated WP uses Manhattan adjacent (≠ scaled Euclid off-axis).
    assert_ne!(xanglexabs(200, 300, 400), xanglexy(200, 300, 400));
}

#[test]
fn saucer1_goto_wp_chases_negated_yaw() {
    let mut g = Game::default();
    g.vars.player_posx = 0;
    g.vars.player_posy = 0;
    g.vars.player_posz = 0;
    let p = alloc(&mut g);
    g.objs.aliens[p as usize].worldx = 0;
    g.objs.aliens[p as usize].worldy = 0;
    g.objs.aliens[p as usize].worldz = 0;
    let idx = alloc(&mut g);
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 800;
        al.worldy = 0;
        al.worldz = 0;
        al.roty = 0;
        al.vel = 0;
    }
    saucer1_istrat(&mut g, idx);
    saucer1_strat(&mut g, idx);
    assert_ne!(
        g.objs.aliens[idx as usize].roty, 0,
        "saucer1 should chase WP yaw"
    );
}
