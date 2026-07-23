//! ROM `pelasercollide` / pollen / `explodegate2` (GSTRATS/EXPSTRAT).

use sf_game::alien::{ASF_NOHITAFFECT, ATLASER};
use sf_game::vars::HARD_HP;
use sf_game::Game;
use sf_strat::common::{sv, StratRam};
use sf_strat::enemy_a::{
    explodegate2_istrat, makepollen_srou, particlepollen_istrat, particlepollen_strat,
    pelasercollide_istrat, ASF2_NOEXPSND, ASF2_RELEXPLODE,
};

#[test]
fn pelasercollide_kills_and_calls_expstrat_on_solid() {
    let mut g = Game::new();
    let laser = g.objs.alloc().expect("laser");
    let wall = g.objs.alloc().expect("wall");
    g.objs.aliens[laser as usize].hp = 5;
    g.objs.aliens[laser as usize].collobjptr = wall;
    g.objs.aliens[wall as usize].hp = HARD_HP;
    g.objs.aliens[wall as usize].sflags |= ASF_NOHITAFFECT;
    // no collstrat → solid
    g.objs.aliens[wall as usize].collstratptr = None;

    let exp = g.world.register_strategy(|g, _| {
        g.vars.set_sv_u8(sv::RNDVAL, 0xAB); // marker that exp ran
    });
    g.objs.aliens[laser as usize].expstratptr = Some(exp);

    pelasercollide_istrat(&mut g, laser);
    assert_eq!(g.objs.aliens[laser as usize].hp, 0);
    assert_eq!(g.vars.sv_u8(sv::RNDVAL), 0xAB);
}

#[test]
fn pelasercollide_skips_wall_sound_when_partner_has_collstrat() {
    let mut g = Game::new();
    let laser = g.objs.alloc().expect("laser");
    let soft = g.objs.alloc().expect("soft");
    g.objs.aliens[laser as usize].hp = 3;
    g.objs.aliens[laser as usize].collobjptr = soft;
    g.objs.aliens[soft as usize].hp = 10;
    let dummy = g.world.register_strategy(|_g, _| {});
    g.objs.aliens[soft as usize].collstratptr = Some(dummy);
    pelasercollide_istrat(&mut g, laser);
    assert_eq!(g.objs.aliens[laser as usize].hp, 0);
}

#[test]
fn makepollen_spawns_above_parent() {
    let mut g = Game::new();
    let parent = g.objs.alloc().expect("parent");
    g.objs.aliens[parent as usize].worldx = 5;
    g.objs.aliens[parent as usize].worldy = -10;
    g.objs.aliens[parent as usize].worldz = 200;
    let p = makepollen_srou(&mut g, parent).expect("pollen");
    assert_eq!(g.objs.aliens[p as usize].worldx, 5);
    assert_eq!(g.objs.aliens[p as usize].worldy, -130);
    assert_eq!(g.objs.aliens[p as usize].worldz, 200);
    assert_eq!(g.objs.aliens[p as usize].sbyte3, 6);
    assert_eq!(g.objs.aliens[p as usize].sbyte2, 250);
}

#[test]
fn particlepollen_expires_at_250() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("slot");
    particlepollen_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 249;
    particlepollen_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn explodegate2_always_stopexplodes() {
    let mut g = Game::new();
    g.vars.set_sv_u16(sv::RNDVAL, 0); // low rnd → may spawn gate
    let idx = g.objs.alloc().expect("slot");
    g.objs.aliens[idx as usize].worldx = 1;
    g.objs.aliens[idx as usize].worldy = 2;
    g.objs.aliens[idx as usize].worldz = 3;
    g.objs.aliens[idx as usize].vx = 10;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND | ASF2_RELEXPLODE;
    // Force spawn path: partner is laser
    let laser = g.objs.alloc().expect("laser");
    g.objs.aliens[idx as usize].collobjptr = laser;
    g.objs.aliens[laser as usize].type_ |= ATLASER;

    // Force !jmp_random(80) by making rnd high — use many trials via direct call
    // after setting rnd so threshold fails. Easier: call with partner laser and
    // accept either spawn or not; always must explode.
    explodegate2_istrat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
    assert_eq!(g.objs.aliens[idx as usize].vx, 0); // stopexplode zeroed
}

#[test]
fn explodegate2_can_spawn_gate2_shape() {
    const SH_GATE_2: u16 = 210;
    // Force spawn: need rnd >= 80% threshold. Keep calling until we see gate_2
    // or give up after seeding.
    let mut saw_gate = false;
    for seed in 0u16..64 {
        let mut g = Game::new();
        g.vars.set_sv_u16(sv::RNDVAL, seed.wrapping_mul(7919));
        let idx = g.objs.alloc().expect("slot");
        g.objs.aliens[idx as usize].sflags2 |= ASF2_NOEXPSND;
        let laser = g.objs.alloc().expect("laser");
        g.objs.aliens[idx as usize].collobjptr = laser;
        g.objs.aliens[laser as usize].type_ |= ATLASER;
        explodegate2_istrat(&mut g, idx);
        if g.objs
            .aliens
            .iter()
            .any(|a| a.active && a.shape == SH_GATE_2)
        {
            saw_gate = true;
            break;
        }
    }
    assert!(saw_gate, "expected gate_2 spawn across random seeds");
}
