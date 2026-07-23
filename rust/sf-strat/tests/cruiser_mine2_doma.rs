//! ROM cruiser1/2 + fall/launcher + updoorcol + mine2 + doma + dpilar(=halfd).

use sf_game::alien::{ASF_COLLDISABLE, ASF_HITFLASH};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::enemies_ground::{
    cruiser1_cont, cruiser1_istrat, cruiser1f_istrat, cruiser1fall_istrat, cruiser1fall_strat,
    cruiser2_istrat, cruiser2fire_istrat, cruiser2launcher_istrat, cruiser2launcher_strat,
    doma_istrat, doma_strat, mine2_istrat, mine2_strat, updoor_istrat, updoor_strat,
    updoorcol_istrat,
};
use sf_strat::enemy_a::{
    dpilar_istrat, halfd_istrat, ASF2_RELEXPLODE, ASF2_SFLAG1, COLLTYPE_ENEMY1, COLLTYPE_ENEMYWEAP,
    DEG180, DEG45, DEG90,
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
    g.objs.aliens[idx as usize].worldz = 1500;
    g.objs.aliens[idx as usize].worldy = -100;
    idx
}

#[test]
fn cruiser1_init_and_fall_tips() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);
    let idx = spawn_obj(&mut g);
    cruiser1_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 30);
    assert_eq!(g.objs.aliens[idx as usize].vel, 20);
    // Same-tick strat: sbyte1==0 + notdelay2 → roty -= 1 from -deg90.
    assert_eq!(
        g.objs.aliens[idx as usize].roty,
        (-(DEG90 as i8) as u8).wrapping_sub(1)
    );
    assert_ne!(g.objs.aliens[idx as usize].sflags2 & ASF2_SFLAG1, 0);
    assert_ne!(g.objs.aliens[idx as usize].collflags & COLLTYPE_ENEMY1, 0);
    let z0 = g.objs.aliens[idx as usize].worldz;
    cruiser1_cont(&mut g, idx);
    // Moved via gen_vecs + playerZ (player at 0, +playerZ may keep z).
    let _ = z0;

    // Fall: tip pitch toward deg45 each tick.
    g.objs.aliens[idx as usize].rotx = 0;
    cruiser1fall_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].rotx, 1);
    for _ in 0..50 {
        cruiser1fall_strat(&mut g, idx);
    }
    assert_eq!(g.objs.aliens[idx as usize].rotx, DEG45);

    // Fighter variant: no sflag1.
    let f = spawn_obj(&mut g);
    g.objs.aliens[f as usize].sflags2 = 0;
    cruiser1f_istrat(&mut g, f);
    assert_eq!(g.objs.aliens[f as usize].sflags2 & ASF2_SFLAG1, 0);
}

#[test]
fn cruiser2_launchers_and_updoorcol() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let body = spawn_obj(&mut g);
    cruiser2fire_istrat(&mut g, body);
    assert_eq!(g.objs.aliens[body as usize].hp, HARD_HP);
    assert_ne!(g.objs.aliens[body as usize].sflags & ASF_COLLDISABLE, 0);
    // Three launcher children attached.
    let kids: Vec<_> = (0..g.objs.aliens.len())
        .filter(|&i| {
            i as u16 != body
                && g.objs.aliens[i].active
                && g.objs.aliens[i].hp == 4
                && g.objs.aliens[i].collflags & COLLTYPE_ENEMY1 != 0
        })
        .collect();
    assert!(kids.len() >= 3, "expected 3 launchers, got {}", kids.len());

    let plain = spawn_obj(&mut g);
    cruiser2_istrat(&mut g, plain);
    assert_eq!(g.objs.aliens[plain as usize].hp, HARD_HP);

    let gun = spawn_obj(&mut g);
    g.objs.aliens[gun as usize].sbyte2 = 1;
    g.objs.aliens[gun as usize].worldx = 0;
    g.objs.aliens[gun as usize].flags |= sf_strat::enemy_a::AF_LEFT_PL; // left of view → cheat path eligible
    cruiser2launcher_istrat(&mut g, gun);
    assert_eq!(g.objs.aliens[gun as usize].hp, 4);
    // Force fire tick: sbyte2=1 → decbne to 0 → fire reset 60
    cruiser2launcher_strat(&mut g, gun);
    assert_eq!(g.objs.aliens[gun as usize].sbyte2, 60);

    let door = spawn_obj(&mut g);
    updoor_istrat(&mut g, door);
    // Far: close anim.
    g.objs.aliens[door as usize].worldz = 2000;
    updoor_strat(&mut g, door);
    assert_eq!(g.objs.aliens[door as usize].animframe, 0);
    // Close: open arm + sbyte1=100 then beqdec → 99
    g.objs.aliens[door as usize].worldz = 200;
    updoor_strat(&mut g, door);
    assert_eq!(g.objs.aliens[door as usize].sbyte1, 99);

    // Col with sbyte1==0 flips the door and sets 5, then hitflash jumps back
    // through updoor_strat in the same frame.  At this close range the normal
    // open path re-arms the collision gate to 100 and beqdec leaves 99.
    g.objs.aliens[door as usize].sbyte1 = 0;
    g.objs.aliens[door as usize].rotz = 0;
    updoorcol_istrat(&mut g, door);
    assert_eq!(g.objs.aliens[door as usize].sbyte1, 99);
    assert_eq!(g.objs.aliens[door as usize].rotz, DEG180);
    assert_ne!(g.objs.aliens[door as usize].sflags & ASF_HITFLASH, 0);
}

#[test]
fn mine2_rises_doma_chases_dpilar_alias() {
    let mut g = Game::new();
    spawn_player(&mut g, 0);

    let mine = spawn_obj(&mut g);
    mine2_istrat(&mut g, mine);
    assert_eq!(g.objs.aliens[mine as usize].hp, 2);
    assert_eq!(g.objs.aliens[mine as usize].vy, -45);
    assert_ne!(g.objs.aliens[mine as usize].sflags2 & ASF2_RELEXPLODE, 0);
    assert_ne!(
        g.objs.aliens[mine as usize].collflags & COLLTYPE_ENEMYWEAP,
        0
    );
    let y0 = g.objs.aliens[mine as usize].worldy;
    mine2_strat(&mut g, mine);
    assert_eq!(g.objs.aliens[mine as usize].vy, -44);
    assert_ne!(g.objs.aliens[mine as usize].worldy, y0);
    // Rising (vy still negative): roty += 12
    let ry = g.objs.aliens[mine as usize].roty;
    let expected_ry = ry; // already spun this tick; next tick spins again
    mine2_strat(&mut g, mine);
    assert_eq!(
        g.objs.aliens[mine as usize].roty,
        expected_ry.wrapping_add(12)
    );

    // Skip to apex: set vy=14 then tick → 15 and explode path.
    g.objs.aliens[mine as usize].vy = 14;
    mine2_strat(&mut g, mine); // vy→15, no explode yet (EQ check before increment)
    assert_eq!(g.objs.aliens[mine as usize].vy, 15);
    // Next tick hits EQ → mine2exp
    let before = g.objs.aliens.len();
    mine2_strat(&mut g, mine);
    // Explode path ran (may spawn beams); stratptr may change.
    let _ = before;

    let doma = spawn_obj(&mut g);
    doma_istrat(&mut g, doma);
    assert_eq!(g.objs.aliens[doma as usize].hp, 2);
    assert_eq!(g.objs.aliens[doma as usize].vel, 30);
    assert_eq!(g.objs.aliens[doma as usize].roty, DEG180);
    // Far enough for xz_dist ≥ 2000 under ROM i16 dist_xz (moderate coords;
    // huge z overflows the approx and falsely reads as "near").
    g.objs.aliens[doma as usize].worldx = 500;
    g.objs.aliens[doma as usize].worldz = 3000;
    g.objs.aliens[doma as usize].vx = 0;
    g.objs.aliens[doma as usize].vy = 0;
    g.objs.aliens[doma as usize].vz = 0;
    doma_strat(&mut g, doma);
    assert_eq!(g.objs.aliens[doma as usize].worldx, 0);
    assert_eq!(g.objs.aliens[doma as usize].worldy, -40);
    // Close: next_state + vx=21 same tick, then vx-- toward ≤19
    g.objs.aliens[doma as usize].worldz = 100;
    g.objs.aliens[doma as usize].stratstate = 0;
    g.objs.aliens[doma as usize].vx = 0;
    doma_strat(&mut g, doma);
    assert_eq!(g.objs.aliens[doma as usize].stratstate, 1);
    assert_eq!(g.objs.aliens[doma as usize].vx, 20); // 21 then −1 same tick

    let pilar = spawn_obj(&mut g);
    dpilar_istrat(&mut g, pilar);
    assert_eq!(g.objs.aliens[pilar as usize].hp, HARD_HP);
    let half = spawn_obj(&mut g);
    halfd_istrat(&mut g, half); // same body as dpilar
}
