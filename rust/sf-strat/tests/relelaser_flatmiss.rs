//! ROM `relelaser` / `relflatmiss` / `flatmiss` + fire_friend/reb/plasma/beamball.

use sf_game::alien::ObjectVisualKind;
use sf_game::Game;
use sf_strat::enemy_a::{
    fire_beamball, fire_friend_elaser, fire_plasma, fire_reb_elaser, flatmiss_istrat,
    flatmiss_strat, relelaser_istrat, relelaser_strat, relflatmiss_istrat, relflatmiss_strat,
    strat_fire_relslowlaserhome, ASF2_SFLAG1, ASF4_RELEXPLODE, SH_BOUNCYBALL,
};

#[test]
fn homing_laser_preserves_firer_speed_and_source_flag_byte() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("firer");
    g.objs.aliens[firer as usize].vel = 60;

    let shot = strat_fire_relslowlaserhome(&mut g, firer, 0, 0).expect("homing laser");
    let laser = g.objs.aliens[shot as usize];

    assert_eq!(laser.sbyte3, 60, "gen_weapon preserves firing-object speed");
    assert_eq!(laser.sflags2, 0, "relexplode is not a source-byte-2 flag");
    assert_ne!(laser.sflags4 & ASF4_RELEXPLODE, 0);
}

#[test]
fn relelaser_istrat_scales_vecs_and_animates() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 66;
        al.sbyte1 = 0;
        al.sbyte2 = 0;
        al.sbyte3 = 20;
        al.count = 10;
        al.sflags2 |= ASF2_SFLAG1;
    }
    relelaser_istrat(&mut g, idx);
    assert!(g.objs.aliens[idx as usize].stratptr.is_some());
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 0);
    assert_eq!(g.objs.aliens[idx as usize].vel, 66);

    relelaser_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].animframe & 0x7F, 2);
    assert_eq!(g.objs.aliens[idx as usize].count, 9);
}

#[test]
fn relelaser_expires_without_numplasers() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("laser");
    g.objs.aliens[idx as usize].vel = 60;
    g.objs.aliens[idx as usize].sbyte3 = 0;
    g.objs.aliens[idx as usize].count = 1;
    g.objs.aliens[idx as usize].sflags2 |= ASF2_SFLAG1;
    relelaser_istrat(&mut g, idx);
    g.objs.aliens[idx as usize].count = 1;
    relelaser_strat(&mut g, idx);
    assert_eq!(g.objs.aldead, 1);
}

#[test]
fn relflatmiss_scrolls_and_kills_on_life() {
    let mut g = Game::new();
    g.vars.pviewvelz = 5;
    let idx = g.objs.alloc().expect("plasma");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 80;
        al.roty = 0;
        al.rotx = 0;
        al.count = 1;
        al.worldz = 100;
        al.sflags2 |= ASF2_SFLAG1;
    }
    relflatmiss_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].snd2, 6);
    // The initializer falls through: the first visit moves and marks death.
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
    assert_eq!(g.objs.aliens[idx as usize].count, 0);
    // The source's two fixed-point cosine multiplies turn speed 80 into 78.
    assert_eq!(g.objs.aliens[idx as usize].worldz, 183);

    // A direct second visit wraps the byte; retirement belongs to the owner.
    relflatmiss_strat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].count, 255);
}

#[test]
fn flatmiss_records_the_zero_lifetime_before_killing() {
    let mut g = Game::new();
    let idx = g.objs.alloc().expect("ball");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.count = 1;
        al.sflags2 |= ASF2_SFLAG1;
    }
    flatmiss_istrat(&mut g, idx);

    assert_eq!(g.objs.aliens[idx as usize].count, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, 0);
}

#[test]
fn flatmiss_does_not_add_player_z() {
    let mut g = Game::new();
    g.vars.pviewvelz = 12;
    let idx = g.objs.alloc().expect("ball");
    {
        let al = &mut g.objs.aliens[idx as usize];
        al.vel = 70;
        al.count = 5;
        al.worldz = 200;
        al.sflags2 |= ASF2_SFLAG1;
    }
    flatmiss_istrat(&mut g, idx);
    assert_eq!(g.objs.aliens[idx as usize].worldz, 268);
    assert_eq!(g.objs.aliens[idx as usize].count, 4);
    let z0 = g.objs.aliens[idx as usize].worldz;
    flatmiss_strat(&mut g, idx);
    // Only velocity applied — no +pviewvelz
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        z0.wrapping_add(g.objs.aliens[idx as usize].vz)
    );
    assert_eq!(g.objs.aliens[idx as usize].count, 3);
}

#[test]
fn flat_weapons_wait_for_their_own_same_pass_visit_and_use_final_caller_aim() {
    use sf_strat::enemy_a::{
        fire_ovalbeam, fire_relovalbeam, fire_relringlaser, fire_ringlaser, fire_shortplasma,
    };
    type Fire = fn(&mut Game, u16) -> Option<u16>;
    const FIRES: [(Fire, u8); 7] = [
        (fire_plasma, 100),
        (fire_beamball, 100),
        (fire_relovalbeam, 100),
        (fire_relringlaser, 100),
        (fire_ovalbeam, 100),
        (fire_ringlaser, 100),
        (fire_shortplasma, 30),
    ];
    fn firer_visit(g: &mut Game, firer: u16) {
        let (fire, life) = FIRES[g.objs.aliens[firer as usize].sbyte1 as usize];
        let shot = fire(g, firer).unwrap();
        assert_eq!(g.objs.aliens[firer as usize].next, Some(shot));
        let object = &mut g.objs.aliens[shot as usize];
        assert_eq!(
            object.count, life,
            "constructor must not run the first visit"
        );
        assert_eq!([object.vx, object.vy, object.vz], [0, 0, 0]);
        assert_eq!(object.snd2, 0);
        // As with source s_fire_weapon callers, finish changing aim and speed
        // after construction. The deferred initializer consumes these values.
        object.rotx = 0;
        object.roty = 64;
        object.vel = 90;
        g.objs.aliens[firer as usize].ptr = shot + 1;
        g.objs.aliens[firer as usize].stratptr = None;
    }
    for (which, (_, life)) in FIRES.into_iter().enumerate() {
        let mut g = Game::new();
        let firer = g.objs.alloc().unwrap();
        let strat = g.world.register_strategy(firer_visit);
        let al = &mut g.objs.aliens[firer as usize];
        al.hp = 20;
        al.sbyte1 = which as u8;
        al.worldx = 100;
        al.worldz = 1000;
        al.stratptr = Some(strat);

        g.run_strategies();

        let shot = g.objs.aliens[firer as usize].ptr - 1;
        let al = g.objs.aliens[shot as usize];
        assert_eq!(al.count, life - 1, "exactly one same-pass visit");
        assert_eq!(al.snd2, 6);
        assert_eq!([al.sbyte1, al.sbyte2], [64, 0]);
        assert_eq!([al.vx, al.vy, al.vz], [-88, 0, 0]);
        assert_eq!(al.worldx, 12);
        g.run_strategies();
        assert_eq!(g.objs.aliens[shot as usize].count, life - 2);
        assert_eq!(g.objs.aliens[shot as usize].worldx, -76);
    }
}

#[test]
fn fire_friend_and_reb_elaser_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("firer");
    g.objs.aliens[firer as usize].vel = 40;

    let friend = fire_friend_elaser(&mut g, firer).expect("friend");
    assert_eq!(g.objs.aliens[friend as usize].ap, 2);
    assert_eq!(g.objs.aliens[friend as usize].vel, 66);
    assert_eq!(g.objs.aliens[friend as usize].count, 10);
    assert!(g.objs.aliens[friend as usize].stratptr.is_some());

    let reb = fire_reb_elaser(&mut g, firer).expect("reb");
    assert_eq!(g.objs.aliens[reb as usize].ap, 2);
    assert_eq!(g.objs.aliens[reb as usize].vel, 60);
    assert_eq!(g.objs.aliens[reb as usize].count, 40);
}

#[test]
fn fire_plasma_and_beamball_stats() {
    let mut g = Game::new();
    let firer = g.objs.alloc().expect("firer");
    g.objs.aliens[firer as usize].vel = 30;

    let plasma = fire_plasma(&mut g, firer).expect("plasma");
    assert_eq!(g.objs.aliens[plasma as usize].shape, SH_BOUNCYBALL);
    assert_eq!(g.objs.aliens[plasma as usize].ap, 10);
    assert_eq!(g.objs.aliens[plasma as usize].vel, 80);
    assert_eq!(g.objs.aliens[plasma as usize].count, 100);
    assert_ne!(g.objs.aliens[plasma as usize].sflags4 & ASF4_RELEXPLODE, 0);
    assert_eq!(
        g.objs.aliens[plasma as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );

    let ball = fire_beamball(&mut g, firer).expect("ball");
    assert_eq!(g.objs.aliens[ball as usize].shape, SH_BOUNCYBALL);
    assert_eq!(g.objs.aliens[ball as usize].ap, 8);
    assert_eq!(g.objs.aliens[ball as usize].vel, 70);
    assert_eq!(
        g.objs.aliens[ball as usize].visual_kind,
        ObjectVisualKind::ScaledSprite
    );
}
