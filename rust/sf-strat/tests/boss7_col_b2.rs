//! Tick 97: boss7 hatch/launcher col, coll, b2, intropart + boss8_strat alias.

use sf_game::alien::{ASF_COLLDISABLE, ASF_COLLIDE, ASF_HITFLASH};
use sf_game::Game;
use sf_strat::bosses::boss8_strat;
use sf_strat::enemy_a::boss_attach_child_to_mother;
use sf_strat::enemy_b::{
    boss7b2_init, boss7b2_strat, boss7coll_istrat, boss7hatchcol_istrat, boss7intropart_istrat,
    boss7launchercol_istrat, strat_boss7_init,
};

const BOSS7_SFLAG_HATCH: u8 = 0x10;
const BOSS7_SFLAG_LAUNCH: u8 = 0x20;
const HF2_MASK: u8 = 0x02;
const DEG180: u8 = 128;
const DEG45: u8 = 32;
const BOSS8_SCALE: i16 = 3;

fn spawn(g: &mut Game) -> u16 {
    let idx = g.objs.alloc().expect("obj");
    g.objs.aliens[idx as usize].active = true;
    idx
}

#[test]
fn attached_boss_parts_run_after_their_mother_in_source_order() {
    let mut game = Game::new();
    let mother = spawn(&mut game);
    let first_child = spawn(&mut game);
    assert!(boss_attach_child_to_mother(
        &mut game,
        mother,
        first_child,
        1
    ));
    let second_child = spawn(&mut game);
    assert!(boss_attach_child_to_mother(
        &mut game,
        mother,
        second_child,
        2
    ));

    assert_eq!(
        game.objs.active_indices(),
        vec![mother, second_child, first_child]
    );
}

#[test]
fn attack_carrier_parts_consume_the_completed_mother_pose_each_tick() {
    const BOSS_START_Y: i16 = -560;
    const BOSS_START_Z: i16 = 3_000;
    const PLAYER_SPEED: i16 = 64;

    let mut game = Game::new();
    let player = spawn(&mut game);
    let boss = spawn(&mut game);
    game.objs.active_move_after(boss, player);
    game.objs.aliens[boss as usize].worldy = BOSS_START_Y;
    game.objs.aliens[boss as usize].worldz = BOSS_START_Z;
    game.vars.internal_playpt = player as i16;
    game.vars.playervel_z = PLAYER_SPEED;
    game.vars.pviewvelz = PLAYER_SPEED;
    strat_boss7_init(&mut game, boss);

    game.run_strategies();
    let mother = game.objs.aliens[boss as usize];
    for child in game
        .objs
        .active_indices()
        .into_iter()
        .filter(|child| game.objs.aliens[*child as usize].ptr == boss + 1)
    {
        let part = game.objs.aliens[child as usize];
        assert_eq!(
            [part.rotx, part.roty, part.rotz],
            [mother.rotx, mother.roty, mother.rotz]
        );
        assert_ne!(
            part.worldz, BOSS_START_Z,
            "child retained the preceding mother pose"
        );
    }
}

#[test]
fn hatchcol_needs_mother_hatch_flag() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let hatch = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, hatch, 1));

    g.objs.aliens[hatch as usize].sflags |= ASF_COLLIDE;
    g.objs.aliens[hatch as usize].hitflags = 0xFF;
    // Mother hatch closed → clear collide/hitflags, no flash
    boss7hatchcol_istrat(&mut g, hatch);
    assert_eq!(g.objs.aliens[hatch as usize].hitflags, 0);
    assert_eq!(g.objs.aliens[hatch as usize].sflags & ASF_COLLIDE, 0);
    assert_eq!(g.objs.aliens[hatch as usize].sflags & ASF_HITFLASH, 0);

    g.objs.aliens[mother as usize].sflags2 |= BOSS7_SFLAG_HATCH;
    g.objs.aliens[hatch as usize].sflags |= ASF_COLLIDE;
    g.objs.aliens[hatch as usize].hitflags = 1;
    g.objs.aliens[hatch as usize].hp = 10;
    boss7hatchcol_istrat(&mut g, hatch);
    assert_ne!(g.objs.aliens[hatch as usize].sflags & ASF_HITFLASH, 0);
}

#[test]
fn launchercol_needs_launch_and_hf2() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let launcher = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, launcher, 2));

    g.objs.aliens[mother as usize].sflags2 |= BOSS7_SFLAG_LAUNCH;
    g.objs.aliens[launcher as usize].sflags |= ASF_COLLIDE;
    g.objs.aliens[launcher as usize].hitflags = 0x01; // no HF2
    boss7launchercol_istrat(&mut g, launcher);
    assert_eq!(g.objs.aliens[launcher as usize].hitflags, 0);
    assert_eq!(g.objs.aliens[launcher as usize].sflags & ASF_COLLIDE, 0);

    g.objs.aliens[launcher as usize].sflags |= ASF_COLLIDE;
    g.objs.aliens[launcher as usize].hitflags = HF2_MASK;
    g.objs.aliens[launcher as usize].hp = 10;
    boss7launchercol_istrat(&mut g, launcher);
    assert_eq!(g.objs.aliens[launcher as usize].hitflags, 0);
    assert_ne!(g.objs.aliens[launcher as usize].sflags & ASF_HITFLASH, 0);
}

#[test]
fn boss7coll_is_hitflash() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    let attacker = spawn(&mut g);
    g.objs.aliens[idx as usize].hp = 10;
    g.objs.aliens[idx as usize].collobjptr = attacker;
    g.objs.aliens[idx as usize].collcount = 1;
    g.objs.aliens[attacker as usize].ap = 1;
    boss7coll_istrat(&mut g, idx);
    assert_ne!(g.objs.aliens[idx as usize].sflags & ASF_HITFLASH, 0);
    assert_eq!(g.objs.aliens[idx as usize].hp, 9);
}

#[test]
fn boss7b2_hide_hatch_then_handoff() {
    let mut g = Game::new();
    let boss = spawn(&mut g);
    let hatch = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, boss, hatch, 1));

    g.objs.aliens[boss as usize].sflags2 |= BOSS7_SFLAG_HATCH;
    g.objs.aliens[boss as usize].roty = DEG180; // already at deg180-deg45? no — chase to 96
    boss7b2_init(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].sflags2 & BOSS7_SFLAG_HATCH, 0);
    assert_eq!(g.objs.aliens[boss as usize].sbyte4, 50);
    assert!(g.objs.aliens[boss as usize].stratptr.is_some());

    // At target yaw: countdown
    g.objs.aliens[boss as usize].roty = DEG180 - DEG45;
    g.objs.aliens[boss as usize].sbyte4 = 2;
    boss7b2_strat(&mut g, boss);
    assert_eq!(g.objs.aliens[boss as usize].sbyte4, 1);

    g.objs.aliens[boss as usize].sbyte4 = 1;
    boss7b2_strat(&mut g, boss);
    // sbyte4→0 → boss7b_init (hatch present → open hatch, sbyte4=30)
    assert_ne!(g.objs.aliens[boss as usize].sflags2 & BOSS7_SFLAG_HATCH, 0);
    assert_eq!(g.objs.aliens[boss as usize].sbyte4, 30);
}

#[test]
fn boss7intropart_childrelpos() {
    let mut g = Game::new();
    let mother = spawn(&mut g);
    let part = spawn(&mut g);
    assert!(boss_attach_child_to_mother(&mut g, mother, part, 4));

    g.objs.aliens[mother as usize].worldx = 100;
    g.objs.aliens[mother as usize].worldy = 200;
    g.objs.aliens[mother as usize].worldz = 300;
    g.objs.aliens[mother as usize].rotx = 10;
    g.objs.aliens[mother as usize].roty = 20;
    g.objs.aliens[mother as usize].rotz = 30;
    g.objs.aliens[part as usize].relposx = 1;
    g.objs.aliens[part as usize].relposy = 2;
    g.objs.aliens[part as usize].relposz = 3;

    boss7intropart_istrat(&mut g, part);
    assert_ne!(g.objs.aliens[part as usize].sflags & ASF_COLLDISABLE, 0);
    // Identity-ish: scale×4 on (1,2,3) with non-zero mother rots — just check moved + rots copied
    assert_eq!(g.objs.aliens[part as usize].rotx, 10);
    assert_eq!(g.objs.aliens[part as usize].roty, 20);
    assert_eq!(g.objs.aliens[part as usize].rotz, 30);
    assert_ne!(g.objs.aliens[part as usize].worldx, 0);
}

#[test]
fn boss8_strat_alias_cont() {
    let mut g = Game::new();
    let idx = spawn(&mut g);
    g.vars.player_posz = 1000;
    g.objs.aliens[idx as usize].sbyte4 = 1;
    g.objs.aliens[idx as usize].hp = 10;
    g.vars.bosshp = 0;
    boss8_strat(&mut g, idx);
    assert_eq!(
        g.objs.aliens[idx as usize].worldz,
        (210i16 << BOSS8_SCALE).wrapping_add(1000)
    );
    assert_eq!(g.objs.aliens[idx as usize].sbyte4, 150); // wrapped + toggle path
    assert_eq!(g.vars.bosshp, 10);
}
