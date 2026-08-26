//! ROM ships + intro1pfall + speedlines + monolithpart + castbit.hit + lspark + door1 inits.

use sf_game::alien::{ASF_COLLDISABLE, ASF_HITFLASH};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::bosses::{castbit_hit_istrat, castbit_istrat};
use sf_strat::enemies_ground::{
    door1closewait_init, door1openwait_init, intro1pfall_istrat, intro1pfall_strat,
    intro1pfalling_init, ships_istrat, ships_strat, speedlines_istrat,
};
use sf_strat::enemy_a::{
    bossflags, monolithpart_istrat, monolithpart_srou, monolithpart_strat, monolithpartl_istrat,
    set_bossflags, ASF2_SFLAG1, BF_FLAG1, BF_FLAG2, BF_FLAG3, COLLTYPE_ENEMY1, DEG90,
};
use sf_strat::player::{lspark_cont, lspark_istrat, lspark_strat};

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
    g.objs.aliens[idx as usize].worldy = -100;
    g.objs.aliens[idx as usize].worldx = 100;
    idx
}

#[test]
fn ships_speedlines_intro_fall() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let ship = spawn_obj(&mut g);
    g.objs.aliens[ship as usize].sword1 = 10;
    g.objs.aliens[ship as usize].sword2 = -5;
    ships_istrat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].count, 70);
    assert_ne!(g.objs.aliens[ship as usize].sflags & ASF_COLLDISABLE, 0);
    let x0 = g.objs.aliens[ship as usize].worldx;
    let z0 = g.objs.aliens[ship as usize].worldz;
    ships_strat(&mut g, ship);
    assert_eq!(g.objs.aliens[ship as usize].count, 69);
    assert_eq!(g.objs.aliens[ship as usize].worldx, x0.wrapping_add(10));
    assert_eq!(g.objs.aliens[ship as usize].worldz, z0.wrapping_add(65 - 5));
    g.objs.aliens[ship as usize].count = 1;
    g.objs.aldead = 0;
    ships_strat(&mut g, ship);
    assert_eq!(g.objs.aldead, 1);

    let streak = spawn_obj(&mut g);
    let sz = g.objs.aliens[streak as usize].worldz;
    speedlines_istrat(&mut g, streak);
    assert_eq!(g.objs.aliens[streak as usize].rotx, DEG90);
    assert_eq!(g.objs.aliens[streak as usize].worldz, sz.wrapping_sub(120));

    let intro = spawn_obj(&mut g);
    g.objs.aliens[intro as usize].sbyte1 = 2;
    g.objs.aliens[intro as usize].sbyte2 = 0;
    g.objs.aliens[intro as usize].rotx = 0;
    intro1pfall_istrat(&mut g, intro);
    intro1pfall_strat(&mut g, intro); // sbyte1 2→1
    assert_eq!(g.objs.aliens[intro as usize].sbyte1, 1);
    intro1pfall_strat(&mut g, intro); // 1→0
    assert_eq!(g.objs.aliens[intro as usize].sbyte1, 0);
    intro1pfall_strat(&mut g, intro); // → falling
    assert!(g.objs.aliens[intro as usize].rotx > 0 || g.objs.aliens[intro as usize].sbyte2 > 0);

    let fall2 = spawn_obj(&mut g);
    intro1pfalling_init(&mut g, fall2);
}

#[test]
fn monolithpart_and_castbit_hit() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    set_bossflags(&mut g, 0);

    let part = spawn_obj(&mut g);
    g.objs.aliens[part as usize].sbyte1 = 1;
    g.objs.aliens[part as usize].vz = 40;
    g.objs.aliens[part as usize].sword1 = 15;
    monolithpartl_istrat(&mut g, part);
    assert_ne!(g.objs.aliens[part as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_eq!(g.objs.aliens[part as usize].sbyte2, 10);
    assert_eq!(g.objs.aliens[part as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[part as usize].collflags & COLLTYPE_ENEMY1, 0);

    // Activate body: sbyte1=1 → decbne to 0 → reset 1, zoom
    g.objs.aliens[part as usize].sbyte1 = 1;
    let z0 = g.objs.aliens[part as usize].worldz;
    monolithpart_strat(&mut g, part);
    assert_eq!(g.objs.aliens[part as usize].sbyte1, 1);
    assert_eq!(
        g.objs.aliens[part as usize].worldz,
        z0.wrapping_add(40) // +vz during zoom (playerZ 0)
    );
    assert_eq!(g.objs.aliens[part as usize].sword1, 14);

    // BF_flag2 → remove
    set_bossflags(&mut g, BF_FLAG2);
    g.objs.aldead = 0;
    monolithpart_strat(&mut g, part);
    assert_eq!(g.objs.aldead, 1);
    set_bossflags(&mut g, 0);

    let mom = spawn_obj(&mut g);
    let before = g.objs.aliens.iter().filter(|a| a.active).count();
    monolithpart_srou(&mut g, mom, 20);
    let after = g.objs.aliens.iter().filter(|a| a.active).count();
    assert!(after > before + 8, "spawned many parts");

    let plain = spawn_obj(&mut g);
    monolithpart_istrat(&mut g, plain);
    assert_eq!(g.objs.aliens[plain as usize].sword1, 15);

    let bit = spawn_obj(&mut g);
    castbit_istrat(&mut g, bit);
    g.objs.aliens[bit as usize].hp = 10;
    castbit_hit_istrat(&mut g, bit);
    assert!(
        g.objs.aliens[bit as usize].hp < 10
            || g.objs.aliens[bit as usize].sflags & ASF_HITFLASH != 0
    );
}

#[test]
fn lspark_and_door1_inits() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let spark = spawn_obj(&mut g);
    g.objs.aliens[spark as usize].count = 3;
    g.objs.aliens[spark as usize].vx = 5;
    lspark_istrat(&mut g, spark);
    assert_eq!(g.objs.aliens[spark as usize].colframe, 0x80);
    let x0 = g.objs.aliens[spark as usize].worldx;
    lspark_strat(&mut g, spark);
    assert_eq!(g.objs.aliens[spark as usize].worldx, x0.wrapping_add(5));
    assert_eq!(g.objs.aliens[spark as usize].count, 2);
    g.objs.aliens[spark as usize].count = 1;
    lspark_cont(&mut g, spark);
    assert_eq!(g.objs.aliens[spark as usize].count, 0);

    let door = spawn_obj(&mut g);
    door1openwait_init(&mut g, door);
    assert_eq!(g.objs.aliens[door as usize].sflags & ASF_COLLDISABLE, 0);
    door1closewait_init(&mut g, door);
    assert_ne!(g.objs.aliens[door as usize].sflags & ASF_COLLDISABLE, 0);

    // silence unused
    let _ = (bossflags(&g), BF_FLAG1, BF_FLAG3);
}
