//! ROM jump0/1 + jump0a + sokuten + item3/6 + core0/1 + rightwall/mine1 + fog.

use sf_game::alien::{ASF_COLLDISABLE, ASF_HITFLASH, ASF_NOHITAFFECT};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemies_ground::{
    item3_istrat, item3_strat, item6_istrat, item6_strat, jump0_istrat, jump0_strat, jump0a_strat,
    jump1_istrat, mine1_istrat, rightwall_istrat, sokuten_istrat, sokuten_strat,
};
use sf_strat::enemy_a::{
    core0_istrat, core0_strat, core1_istrat, core1_strat, core1col_istrat, fog_strat, gasflags,
    set_gasflags, wm, COLLTYPE_ENEMY1, DEG180, DEG90,
};

fn spawn_player(g: &mut Game, z: i16) {
    let p = g.objs.alloc().expect("player");
    assert_eq!(p, 0);
    g.objs.aliens[0].active = true;
    g.objs.aliens[0].worldz = z;
    g.objs.aliens[0].worldx = 0;
    g.objs.aliens[0].worldy = -40;
    g.objs.aliens[0].sflags3 |= sf_game::alien::ASF3_REALOBJ;
    g.vars.player_posx = 0;
    g.vars.player_posy = -40;
    g.vars.player_posz = z;
    g.vars.internal_playpt = 0;
}

fn spawn_obj(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    g.objs.aliens[idx as usize].worldz = 2000;
    g.objs.aliens[idx as usize].worldy = -50;
    idx
}

#[test]
fn jump1_static_hard() {
    let mut g = Game::new();
    let idx = spawn_obj(&mut g);
    jump1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, HARD_HP);
    assert_eq!(g.objs.aliens[idx as usize].ap, 8);
    assert_eq!(g.objs.aliens[idx as usize].roty, DEG180);
    assert!(g.objs.aliens[idx as usize].stratptr.is_none());
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
}

#[test]
fn jump0_waits_then_fires_at_apex() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    jump0_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 2);
    assert_eq!(g.objs.aliens[idx as usize].ap, 4);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);

    // Far: stay in jump0.
    g.objs.aliens[idx as usize].worldz = 2000;
    jump0_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].vy, 0);

    // Close → jump0a same tick (vy=-30 then falldown +2 → -28).
    g.objs.aliens[idx as usize].worldz = 500;
    jump0_strat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].vy < 0);

    // Ascend until above -90 so coll clears, then fall to fire at vy>=0.
    g.objs.aliens[idx as usize].worldy = -120;
    g.objs.aliens[idx as usize].vy = -2;
    jump0a_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sflags & ASF_COLLDISABLE, 0);

    g.objs.aliens[idx as usize].vy = 0;
    g.objs.aliens[idx as usize].sflags2 &= !sf_strat::enemy_a::ASF2_SFLAG1;
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    jump0a_strat(&mut g, idx);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before, "HMISSILE1 spawned at apex");
    assert_ne!(
        g.objs.aliens[idx as usize].sflags2 & sf_strat::enemy_a::ASF2_SFLAG1,
        0
    );
}

#[test]
fn sokuten_turns_heading() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    sokuten_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 16);
    assert_eq!(g.objs.aliens[idx as usize].ap, 16);
    assert_eq!(g.objs.aliens[idx as usize].vel, 30);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, 0u8.wrapping_sub(DEG90));

    g.objs.aliens[idx as usize].sbyte2 = 0; // .doturn path
    let h0 = g.objs.aliens[idx as usize].sbyte1;
    sokuten_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].sbyte1, h0.wrapping_sub(1));
    assert!(g.objs.aliens[idx as usize].collstratptr.is_some());
}

#[test]
fn item3_heals_body_item6_wireship() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    // Body pcbox proxy.
    let box_idx = g.objs.alloc().expect("pcbox");
    g.objs.aliens[box_idx as usize].active = true;
    g.objs.aliens[box_idx as usize].hp = 10;
    g.vars.write_ext16(wm::PCBOXOBJ_B, box_idx);

    let idx = spawn_obj(&mut g);
    g.objs.aliens[idx as usize].worldz = 50;
    g.objs.aliens[idx as usize].worldx = 0;
    g.objs.aliens[idx as usize].worldy = -40;
    item3_istrat(&mut g, idx);
    item3_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[box_idx as usize].hp, 15);
    // Switched into flashplayer (count=20).
    assert_eq!(g.objs.aliens[idx as usize].count, 20);

    let w = spawn_obj(&mut g);
    g.objs.aliens[w as usize].worldz = 50;
    g.objs.aliens[w as usize].worldx = 0;
    g.objs.aliens[w as usize].worldy = -40;
    item6_istrat(&mut g, w);
    // Same-tick pickup if already close.
    assert_ne!(g.vars.pshipflags2 & 2, 0); // PSF2_WIRESHIP
    assert_eq!(g.vars.shieldup, 1);
    assert_eq!(g.objs.aldead, 1);
    let _ = item6_strat; // pub
}

#[test]
fn core0_1_rightwall_mine_fog() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let c0 = spawn_obj(&mut g);
    core0_istrat(&mut g, c0);
    assert_eq!(g.objs.aliens[c0 as usize].hp, HARD_HP);
    assert_eq!(gasflags(&g) & 0x08, 0);
    let gf = gasflags(&g) | 0x08;
    set_gasflags(&mut g, gf);
    core0_strat(&mut g, c0);
    assert_ne!(g.objs.aliens[c0 as usize].sflags & ASF_HITFLASH, 0);
    assert_eq!(gasflags(&g) & 0x08, 0);

    let c1 = spawn_obj(&mut g);
    g.objs.aliens[c1 as usize].worldz = 2000;
    core1_istrat(&mut g, c1);
    assert_eq!(g.objs.aliens[c1 as usize].hp, 6);
    assert_ne!(g.objs.aliens[c1 as usize].sflags & ASF_NOHITAFFECT, 0);
    core1_strat(&mut g, c1);
    // Far: still nohitaffect.
    assert_ne!(g.objs.aliens[c1 as usize].sflags & ASF_NOHITAFFECT, 0);
    g.objs.aliens[c1 as usize].worldz = 100;
    core1_strat(&mut g, c1);
    assert_eq!(g.objs.aliens[c1 as usize].sflags & ASF_NOHITAFFECT, 0);
    let ry = g.objs.aliens[c1 as usize].roty;
    assert_eq!(ry, DEG180.wrapping_add(8).wrapping_add(8)); // two ticks of +8

    core1col_istrat(&mut g, c1);
    assert_ne!(gasflags(&g) & 0x08, 0);

    let w = spawn_obj(&mut g);
    rightwall_istrat(&mut g, w);
    assert_ne!(g.objs.aliens[w as usize].sflags & ASF_COLLDISABLE, 0);

    mine1_istrat(&mut g, w);
    fog_strat(&mut g, w);
}
