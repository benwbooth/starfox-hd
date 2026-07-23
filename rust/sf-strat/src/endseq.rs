//! Route-specific end-sequence boss recap object construction.
//!
//! `ENDSEQ.ASM` records semantic encounter entries during the campaign, then
//! rebuilds each encounter in a clean object lane. This module performs that
//! construction with ordinary typed objects and the already-ported Rust
//! strategies; it does not expose source addresses or a byte-memory facade.

use sf_game::game::StrategyFn;
use sf_game::vars::BossEncounter;
use sf_game::Game;

use crate::common::{initgame_strats_l, strat_make_obj};

const SHAPE_NULL: u16 = 0;
const SHAPE_BARRICADER: u16 = 19;
const SHAPE_NUCLEUS_BEAM: u16 = 43;
const SHAPE_NUCLEUS_PILLAR: u16 = 45;
const SHAPE_NUCLEUS_CORE: u16 = 46;
const SHAPE_ATOMIC_BASE: u16 = 57;
const SHAPE_CASTING_MACHINE: u16 = 69;
const SHAPE_CRUSHER: u16 = 76;
const SHAPE_DANCING_INSECT: u16 = 77;
const SHAPE_HIGHWAY_MACHINE: u16 = 78;
const SHAPE_GREAT_COMMANDER: u16 = 81;
const SHAPE_WEB: u16 = 84;
const SHAPE_AIRSHIP: u16 = 94;
const SHAPE_GROUND_BOSS: u16 = 120;
const SHAPE_FINAL_FACE: u16 = 223;
const SHAPE_ROCK_CRUSHER: u16 = 300;
const SHAPE_PHANTRON: u16 = 422;
const SHAPE_ARM_THROWER: u16 = 11;

const BOSS8_LAUNCHER_RADIUS: i16 = 1_200;
const BOSS8_CORE_DEPTH: i16 = 1_680;
const BOSS8_PILLAR_HEIGHT: i16 = 400;
const BOSS8_LAUNCHER_ANGLES: [u8; 4] = [80, 112, 176, 240];
const BOSS8_PILLAR_ANGLE_STEP: u8 = 32;
const BOSS8_PILLAR_COUNT: u8 = 8;

#[derive(Debug, Clone, Copy)]
struct ReplayObject {
    x: i16,
    y: i16,
    z: i16,
    shape: u16,
    init: StrategyFn,
}

fn spawn(g: &mut Game, object: ReplayObject) -> Option<u16> {
    let index = strat_make_obj(g, object.shape)?;
    let init = g.world.register_strategy(object.init);
    let alien = &mut g.objs.aliens[index as usize];
    alien.worldx = object.x;
    alien.worldy = object.y;
    alien.worldz = object.z;
    alien.stratptr = Some(init);
    Some(index)
}

fn spawn_nucleus_replay(g: &mut Game) -> Option<u16> {
    let anchor = spawn(
        g,
        ReplayObject {
            x: 0,
            y: 0,
            z: BOSS8_CORE_DEPTH,
            shape: SHAPE_NUCLEUS_CORE,
            init: crate::bosses::strat_boss8_init,
        },
    )?;

    for angle in BOSS8_LAUNCHER_ANGLES {
        let Some(index) = spawn(
            g,
            ReplayObject {
                x: 0,
                y: 0,
                z: BOSS8_LAUNCHER_RADIUS,
                shape: SHAPE_NUCLEUS_BEAM,
                init: crate::bosses::nucleuslauncher_istrat,
            },
        ) else {
            continue;
        };
        g.objs.aliens[index as usize].sbyte2 = angle;
    }

    let pillar_init = g.world.istrats[crate::bosses::IS_NUCLEUSPILLAR];
    for pillar in 0..BOSS8_PILLAR_COUNT {
        let Some(index) = strat_make_obj(g, SHAPE_NUCLEUS_PILLAR) else {
            continue;
        };
        let alien = &mut g.objs.aliens[index as usize];
        alien.worldy = BOSS8_PILLAR_HEIGHT;
        alien.worldz = BOSS8_LAUNCHER_RADIUS;
        alien.sbyte2 = BOSS8_PILLAR_ANGLE_STEP.wrapping_mul(pillar);
        alien.stratptr = pillar_init;
    }
    Some(anchor)
}

/// Construct one recorded recap encounter and return its camera anchor.
pub fn spawn_replay_boss(g: &mut Game, encounter: BossEncounter) -> Option<u16> {
    use BossEncounter::*;

    initgame_strats_l(g);
    if matches!(encounter, Route1Stage3 | Route3Stage4) {
        return spawn_nucleus_replay(g);
    }

    let object = match encounter {
        Route1Stage1 | Route2Stage1 => ReplayObject {
            x: 0,
            y: -560,
            z: -200,
            shape: SHAPE_PHANTRON,
            init: crate::enemy_b::strat_boss7_init,
        },
        Route1Stage2 | Route2Stage2 => ReplayObject {
            x: 0,
            y: 0,
            z: 2_500,
            shape: SHAPE_BARRICADER,
            init: crate::enemy_a::strat_boss1_init,
        },
        Route1Stage4 => ReplayObject {
            x: 2_000,
            y: -600,
            z: 1_000,
            shape: SHAPE_ROCK_CRUSHER,
            init: crate::bossh::bossh_init,
        },
        Route1Stage5 => ReplayObject {
            x: 0,
            y: -1_000,
            z: 2_500,
            shape: SHAPE_CRUSHER,
            init: crate::bossb::bossbrob_init,
        },
        Route1Stage6 => ReplayObject {
            x: 0,
            y: -1_000,
            z: 2_500,
            shape: SHAPE_CRUSHER,
            init: crate::bossb::bossbrobdemo_istrat,
        },
        Route2Stage3 => ReplayObject {
            x: 0,
            y: -60,
            z: 0,
            shape: SHAPE_GROUND_BOSS,
            init: crate::ground::strat_gnd_init,
        },
        Route2Stage4 => ReplayObject {
            x: 0,
            y: -80,
            z: 2_000,
            shape: SHAPE_ARM_THROWER,
            init: crate::bosses::strat_flingboss_init,
        },
        Route2Stage5 => ReplayObject {
            x: 0,
            y: 0,
            z: 2_000,
            shape: SHAPE_NULL,
            init: crate::bosses::strat_castanet_init,
        },
        Route2Stage6 => ReplayObject {
            x: -200,
            y: -70,
            z: -300,
            shape: SHAPE_HIGHWAY_MACHINE,
            init: crate::bosses::madtrucker_init,
        },
        Route3Stage1 => ReplayObject {
            x: 3_000,
            y: 0,
            z: 1_500,
            shape: SHAPE_ATOMIC_BASE,
            init: crate::enemy_b::strat_bossa_init,
        },
        Route3Stage2 => ReplayObject {
            x: 0,
            y: 0,
            z: 1_200,
            shape: SHAPE_WEB,
            init: crate::bosses::strat_webmonster_init,
        },
        Route3Stage3 => ReplayObject {
            x: -100,
            y: 0,
            z: 2_500,
            shape: SHAPE_DANCING_INSECT,
            init: crate::bosses::strat_chicken_init,
        },
        Route3Stage5 => ReplayObject {
            x: 0,
            y: 0,
            z: 2_500,
            shape: SHAPE_CASTING_MACHINE,
            init: crate::bosses::strat_boss2_init,
        },
        Route3Stage6 => ReplayObject {
            x: 0,
            y: 2_000,
            z: 2_500,
            shape: SHAPE_GREAT_COMMANDER,
            init: crate::enemy_b::strat_bossf_init,
        },
        Route3Stage7 => ReplayObject {
            x: -100,
            y: -500,
            z: 0,
            shape: SHAPE_AIRSHIP,
            init: crate::bossf_heli::airship_istrat,
        },
        FinalBattle => ReplayObject {
            x: 0,
            y: -60,
            z: -200,
            shape: SHAPE_FINAL_FACE,
            init: crate::enemy_a::monolith_istrat,
        },
        Route1Stage3 | Route3Stage4 => unreachable!("nucleus replay handled above"),
    };
    spawn(g, object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_game::alien::ASF4_PLAYEROBJ;

    fn game() -> Game {
        let mut game = Game::new();
        crate::table::register_all(&mut game);
        let player = strat_make_obj(&mut game, SHAPE_NULL).expect("player slot");
        game.objs.aliens[player as usize].sflags4 |= ASF4_PLAYEROBJ;
        game.vars.internal_playpt = player as i16;
        game
    }

    #[test]
    fn every_semantic_encounter_builds_a_live_anchor() {
        use BossEncounter::*;
        let encounters = [
            Route1Stage1,
            Route1Stage2,
            Route1Stage3,
            Route1Stage4,
            Route1Stage5,
            Route1Stage6,
            Route2Stage1,
            Route2Stage2,
            Route2Stage3,
            Route2Stage4,
            Route2Stage5,
            Route2Stage6,
            Route3Stage1,
            Route3Stage2,
            Route3Stage3,
            Route3Stage4,
            Route3Stage5,
            Route3Stage6,
            Route3Stage7,
            FinalBattle,
        ];

        for encounter in encounters {
            let mut game = game();
            let anchor = spawn_replay_boss(&mut game, encounter).expect("recap anchor");
            assert!(game.objs.aliens[anchor as usize].active, "{encounter:?}");
            assert!(
                game.objs.aliens[anchor as usize].stratptr.is_some(),
                "{encounter:?}"
            );
        }
    }

    #[test]
    fn nucleus_replay_builds_the_source_thirteen_object_display() {
        let mut game = game();
        let _ = spawn_replay_boss(&mut game, BossEncounter::Route1Stage3);
        let recap_objects = game
            .objs
            .aliens
            .iter()
            .filter(|alien| alien.active && alien.sflags4 & ASF4_PLAYEROBJ == 0)
            .count();
        assert_eq!(recap_objects, 13);
    }
}
