//! ROM `nuke` / `nukeexp` / `removenuke` / `fire_nuke` (GSTRATS.ASM).

use sf_game::alien::{ASF3_REALOBJ, ASF_HITFLASH, ASF_NOHITAFFECT, ATNUKED};
use sf_game::vars::PSF_NOFIRE;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    fire_nuke, missbound_chk_exp, nuke_istrat, nuke_strat, nukeexp_istrat, nukeexp_strat,
    removenuke_istrat, ASF2_SFLAG1, NUKE_AP, NUKE_MAX_RADIUS, NUKE_RATE,
};

#[test]
fn fire_nuke_spawns_with_rom_stats() {
    let mut g = Game::new();
    let player = g.objs.alloc().expect("player");
    g.objs.aliens[player as usize].vel = 40;
    g.objs.aliens[player as usize].worldz = 100;
    let shot = fire_nuke(&mut g, player).expect("nuke");
    assert_eq!(g.objs.aliens[shot as usize].hp, 2);
    assert_eq!(g.objs.aliens[shot as usize].ap, 8);
    assert_eq!(g.objs.aliens[shot as usize].vel, 50);
    assert_eq!(g.objs.aliens[shot as usize].count, 28);
    assert_eq!(g.objs.aliens[shot as usize].sbyte3, 40);
    assert!(g.objs.aliens[shot as usize].stratptr.is_some());
    assert!(g.objs.aliens[shot as usize].expstratptr.is_some());
}

#[test]
fn removenuke_refunds_specwep_and_removes() {
    let mut g = Game::new();
    g.vars.set_sv_u16(sv::SPECWEPCNT, 2);
    let idx = g.objs.alloc().expect("nuke");
    removenuke_istrat(&mut g, idx);
    assert_eq!(g.vars.sv_u16(sv::SPECWEPCNT), 3);
    assert_eq!(g.vars.sv_u8(sv::SPECIALDELAY), 1);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn nuke_strat_removes_when_nofire() {
    let mut g = Game::new();
    g.vars.set_sv_u16(sv::SPECWEPCNT, 1);
    let idx = g.objs.alloc().expect("nuke");
    g.objs.aliens[idx as usize].count = 10;
    g.objs.aliens[idx as usize].sbyte3 = 0;
    nuke_istrat(&mut g, idx);
    g.vars.pshipflags |= PSF_NOFIRE;
    nuke_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.vars.sv_u16(sv::SPECWEPCNT), 2); // refunded
}

#[test]
fn nuke_strat_counts_down_and_kills() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("nuke");
    g.objs.aliens[idx as usize].count = 1;
    g.objs.aliens[idx as usize].sbyte3 = 0;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1; // skip missbound
    nuke_istrat(&mut g, idx);
    nuke_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
}

#[test]
fn nukeexp_damages_front_realobjs_in_ring() {
    let mut g = Game::new();
    let nuke = g.objs.alloc().expect("nuke");
    g.objs.aliens[nuke as usize].worldx = 0;
    g.objs.aliens[nuke as usize].worldz = 0;
    nukeexp_istrat(&mut g, nuke);
    assert_eq!(g.objs.aliens[nuke as usize].sword1, NUKE_RATE);
    assert_eq!(g.vars.sv_u16(sv::CIRCLEOBJ), nuke.wrapping_add(1));
    assert_ne!(g.vars.circleanim, 0);

    let victim = g.objs.alloc().expect("victim");
    {
        let al = &mut g.objs.aliens[victim as usize];
        al.worldx = 50;
        al.worldz = 0;
        al.hp = 20;
        al.sflags3 |= ASF3_REALOBJ;
        al.flags |= 8; // AF_FRONT_PL
    }
    // First ring: [0, 200) — victim at dist ~50 is inside.
    nukeexp_strat(&mut g, nuke);
    assert_eq!(g.objs.aliens[victim as usize].hp, 20 - NUKE_AP);
    assert_ne!(g.objs.aliens[victim as usize].sflags & ASF_HITFLASH, 0);
    assert_ne!(g.objs.aliens[victim as usize].type_ & ATNUKED, 0);
    assert_eq!(g.objs.aliens[nuke as usize].sword1, NUKE_RATE * 2);
}

#[test]
fn nukeexp_skips_nohitaffect_and_removes_at_max() {
    let mut g = Game::new();
    let nuke = g.objs.alloc().expect("nuke");
    nukeexp_istrat(&mut g, nuke);
    let immune = g.objs.alloc().expect("immune");
    {
        let al = &mut g.objs.aliens[immune as usize];
        al.worldx = 10;
        al.hp = 50;
        al.sflags3 |= ASF3_REALOBJ;
        al.flags |= 8;
        al.sflags |= ASF_NOHITAFFECT;
    }
    nukeexp_strat(&mut g, nuke);
    assert_eq!(g.objs.aliens[immune as usize].hp, 50);

    g.objs.aliens[nuke as usize].sword1 = NUKE_MAX_RADIUS;
    nukeexp_strat(&mut g, nuke);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn missbound_kills_past_right_edge() {
    let mut g = Game::new();
    g.vars.set_sv_u8(sv::MISSBOUNDFLAGS, 2); // MB_RIGHT
    g.vars.set_sv_i16(sv::MAXMMOVEX, 100);
    let idx = g.objs.alloc().expect("w");
    g.objs.aliens[idx as usize].worldx = 101;
    g.objs.aliens[idx as usize].hp = 5;
    missbound_chk_exp(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
}
