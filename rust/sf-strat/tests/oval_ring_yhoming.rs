//! ROM oval/ring/shortplasma fire + elaser + Yhoming / fire_YHplasma.

use sf_game::alien::ATZREMOVE;
use sf_game::Game;
use sf_strat::enemy_a::{
    elaser_istrat, elaser_strat, fire_hplasma, fire_ovalbeam, fire_relovalbeam, fire_relringlaser,
    fire_ringlaser, fire_shortplasma, fire_slow_elaser, fire_yhplasma, yhoming_istrat,
    yhoming_strat, ASF2_RELEXPLODE, ASF2_SFLAG1, SH_BOUNCYBALL,
};

#[test]
fn fire_rel_and_abs_oval_ring_shortplasma() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].vel = 25;

    let a = fire_relovalbeam(&mut g, firer).expect("reloval");
    assert_eq!(g.objs.aliens[a as usize].shape, 416);
    assert_eq!(g.objs.aliens[a as usize].ap, 8);
    assert_eq!(g.objs.aliens[a as usize].vel, 70);
    assert_eq!(g.objs.aliens[a as usize].count, 100);

    let b = fire_relringlaser(&mut g, firer).expect("relring");
    assert_eq!(g.objs.aliens[b as usize].shape, 334);
    assert_eq!(g.objs.aliens[b as usize].ap, 6);

    let c = fire_ovalbeam(&mut g, firer).expect("oval");
    assert_eq!(g.objs.aliens[c as usize].shape, 416);
    assert_eq!(g.objs.aliens[c as usize].ap, 8);

    let d = fire_ringlaser(&mut g, firer).expect("ring");
    assert_eq!(g.objs.aliens[d as usize].shape, 334);
    assert_eq!(g.objs.aliens[d as usize].ap, 6);

    let e = fire_shortplasma(&mut g, firer).expect("short");
    assert_eq!(g.objs.aliens[e as usize].shape, SH_BOUNCYBALL);
    assert_eq!(g.objs.aliens[e as usize].ap, 10);
    assert_eq!(g.objs.aliens[e as usize].count, 30);
    assert_ne!(g.objs.aliens[e as usize].sflags2 & ASF2_RELEXPLODE, 0);
}

#[test]
fn fire_hplasma_exact_shape_stats_immunity_and_homing() {
    let mut g = Game::new();
    let player = g.objs.alloc().expect("p");
    assert_eq!(player, 0);
    g.objs.aliens[player as usize].active = true;
    g.objs.aliens[player as usize].worldx = 1000;
    g.objs.aliens[player as usize].worldz = 2000;
    let firer = g.objs.alloc().expect("f");
    g.objs.aliens[firer as usize].active = true;

    let shot = fire_hplasma(&mut g, firer).expect("hplasma");
    {
        let al = &mut g.objs.aliens[shot as usize];
        al.ptr = player + 1;
        al.fireobjptr = player + 1;
    }
    let al = g.objs.aliens[shot as usize];
    assert_eq!(al.shape, SH_BOUNCYBALL);
    assert_eq!(al.hp, 1);
    assert_eq!(al.ap, 10);
    assert_eq!(al.vel, 60);
    assert_eq!(al.count, 50);
    assert_eq!(al.snd2, 6);
    assert_eq!(al.immuneptr, firer);
    assert_eq!(g.objs.aliens[firer as usize].immuneptr, shot);

    let initial_aim = (al.sbyte1, al.sbyte2);
    g.call_strat(al.stratptr.expect("homingflat tick"), shot);
    let al = g.objs.aliens[shot as usize];
    assert_ne!((al.sbyte1, al.sbyte2), initial_aim);
    assert_eq!(al.count, 49);
}

#[test]
fn elaser_moves_without_player_z_and_expires() {
    let mut g = Game::new();
    g.vars.pviewvelz = 20;
    let idx = g.objs.alloc().expect("laser");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 60;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.count = 2;
        al.worldz = 100;
        al.sflags2 |= ASF2_SFLAG1;
    }
    elaser_istrat(&mut g, idx);
    let z0 = g.objs.aliens[idx as usize].worldz;
    elaser_strat(&mut g, idx);
    // No add_player_z — only velocity
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_add(g.objs.aliens[idx as usize].vz)
    );
    assert_eq!(g.objs.aliens[idx as usize].count, 1);
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 2);

    elaser_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn fire_slow_elaser_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("f");
    let shot = fire_slow_elaser(&mut g, firer).expect("slow");
    assert_eq!(g.objs.aliens[shot as usize].vel, 60);
    assert_eq!(g.objs.aliens[shot as usize].count, 40);
    assert_eq!(g.objs.aliens[shot as usize].ap, 2);
}

#[test]
fn yhoming_snaps_yaw_toward_ptr_and_animates() {
    let mut g = Game::new();
    let target = g.objs.alloc().expect("t");
    g.objs.aliens[target as usize].worldx = 500;
    g.objs.aliens[target as usize].worldz = 0;
    let idx = g.objs.alloc().expect("yh");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.worldx = 0;
        al.worldz = 0;
        al.vel = 100;
        al.count = 5;
        al.ptr = target.wrapping_add(1);
        al.sflags2 |= ASF2_SFLAG1;
    }
    yhoming_istrat(&mut g, idx);
    let anim0 = g.objs.aliens[idx as usize].animframe & 0x7F;
    assert!(anim0 < 8);
    yhoming_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].animframe & 0x7F,
        (anim0 + 1) % 8
    );
    // `s_obj2obj_angle` stores the negated Yanglexy result because the ROM's
    // 3-D vector generator indexes yaw with the opposite sign.
    let yaw = g.objs.aliens[idx as usize].roty;
    let expected = sf_core::aim_angle::yanglexy(500, 0).wrapping_neg();
    assert_eq!(yaw, expected);
    assert_eq!(g.objs.aliens[idx as usize].count, 4);
}

#[test]
fn fire_yhplasma_clears_zremove_and_homes() {
    let mut g = Game::new();
    let player = g.objs.alloc().expect("p");
    assert_eq!(player, 0);
    let firer = g.objs.alloc().expect("f");
    let shot = fire_yhplasma(&mut g, firer).expect("yh");
    assert_eq!(g.objs.aliens[shot as usize].vel, 100);
    assert_eq!(g.objs.aliens[shot as usize].count, 50);
    assert_eq!(g.objs.aliens[shot as usize].type_ & ATZREMOVE, 0);
    assert_eq!(g.objs.aliens[shot as usize].ptr, 1); // player+1
}
