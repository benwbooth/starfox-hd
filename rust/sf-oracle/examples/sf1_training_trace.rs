//! Strict retail/native trace for the complete first Training course cycle.

use sf_core::pad;
use sf_game::shell::{GameState, GameplayEntryPhase, Shell};
use sf_oracle::{
    load_retail_rom, RetailMachine, AL_ROTX, AL_ROTY, AL_ROTZ, AL_SBYTE3, AL_SWORD2, AL_VEL,
    RETAIL_DOSTRATS, RETAIL_GAMEFLAGS, RETAIL_GAMEFRAME, RETAIL_LASTPLAYZ, RETAIL_LASTZCHANGE,
    RETAIL_MAPCNT, RETAIL_POOL, RETAIL_PVIEWPOSZ, RETAIL_PVIEWVELZ, RETAIL_RAND, RETAIL_SHAPES,
    RETAIL_VIEW_POSITION_X, RETAIL_VIEW_POSITION_Y, RETAIL_VIEW_POSITION_Z,
};
use std::collections::BTreeSet;

const WORK_RAM: u32 = 0x7E_0000;
const RETAIL_MESSAGE_COUNT: u32 = 0x189D;
const RETAIL_MESSAGE_FACE_PHASE: u32 = 0x189E;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const TRAINING_CONFIRM_END_TICK: u32 = 420;
const PROBE_END_TICK: u32 = 2_200;
const SOURCE_SHAPE_CATALOG_ENTRIES: u16 = 512;
const STARTUP_ROLE_SLOTS: u16 = 5;
const RETAIL_VIEW_FLOAT_CURSOR: u32 = 0x14E4;
const RETAIL_VIEW_FLOAT_Y: u32 = 0x14E8;
const REQUIRED_TRAINING_SHAPES: [(u16, &str); 13] = [
    (sf_map::levels::route2::rc::SH_ZACO_5, "zaco_5"),
    (sf_map::levels::route2::rc::SH_BU_8, "bu_8"),
    (sf_map::levels::route2::rc::SH_BU_1, "bu_1"),
    (sf_map::levels::route2::rc::SH_PILON, "pilon"),
    (sf_map::levels::route2::rc::SH_BU_0, "bu_0"),
    (sf_map::levels::route2::rc::SH_BU_2, "bu_2"),
    (sf_map::levels::route2::rc::SH_TOWER_2, "tower_2"),
    (sf_map::levels::route2::rc::SH_TRAINING, "training_ring"),
    (sf_map::levels::route2::rc::SH_PILLAR3, "pillar3"),
    (sf_map::levels::route2::rc::SH_ROBOT_0, "robot_0"),
    (sf_map::levels::route2::rc::SH_BU_7, "bu_7"),
    (sf_map::levels::route2::rc::SH_BASE_1, "base_1"),
    (sf_map::levels::route2::rc::SH_FRIENDSHIP_4, "friendship_4"),
];
const DIRECT_SHAPE_IDS: [(u16, u16); 14] = [
    (0xDD30, 298), // pilon
    (0xBD40, 482), // training ring
    (0xB075, 479), // large laser flash
    (0xB289, 367), // spark explosion
    (0xB2A5, 342), // laser death flash
    (0xB2C1, 380), // line spark
    (0xB11D, 462), // medium explosion sprite
    (0xBE04, 466), // medium explosion polygons
    (0xB101, 461), // small explosion sprite
    (0xB587, 465), // small explosion polygons
    (0xACF5, 2),   // medium explosion envelope
    (0xADD5, 357), // smoke
    (0xBB9C, 420), // robot_0
    (0xC360, 351), // my_w
];

fn configured_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    shell.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    shell.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    shell.set_shape_extents(sf_render::shapes::sf1_shape_half_extents());
    shell
}

fn scripted_input(tick: u32) -> u16 {
    if tick <= TRAINING_CONFIRM_END_TICK
        && tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS
    {
        pad::START
    } else {
        0
    }
}

fn retail_flat_shape(retail: &RetailMachine, source_word: u16) -> u16 {
    if let Some((_, native_shape)) = DIRECT_SHAPE_IDS
        .iter()
        .find(|(retail_shape, _)| *retail_shape == source_word)
    {
        return *native_shape;
    }
    (0..SOURCE_SHAPE_CATALOG_ENTRIES)
        .find(|catalog_id| retail.peek16(RETAIL_SHAPES + u32::from(*catalog_id) * 2) == source_word)
        .map(sf_core::shape::resolve_shape_word)
        .unwrap_or_else(|| sf_core::shape::resolve_shape_word(source_word))
}

fn assert_level_state(retail: &RetailMachine, native: &Shell, tick: u32) {
    assert_eq!(
        native.game.vars.gameframe,
        retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
        "Training game frame at tick {tick}"
    );
    assert_eq!(
        native.game.vars.mapcnt,
        retail.peek16(WORK_RAM | RETAIL_MAPCNT),
        "Training map countdown at tick {tick}"
    );
    assert_eq!(
        native.game.world.lastplayz,
        retail.peek16(WORK_RAM | RETAIL_LASTPLAYZ) as i16,
        "Training previous player depth at tick {tick}"
    );
    assert_eq!(
        native.game.world.lastzchange,
        retail.peek16(WORK_RAM | RETAIL_LASTZCHANGE) as i16,
        "Training player depth change at tick {tick}"
    );
    assert_eq!(
        native.game.vars.pviewvelz,
        retail.peek16(WORK_RAM | RETAIL_PVIEWVELZ) as i16,
        "Training forward velocity at tick {tick}"
    );
    assert_eq!(
        native.game.vars.strategy.player_view_position[2],
        retail.peek16(WORK_RAM | RETAIL_PVIEWPOSZ) as i16,
        "Training player-view depth at tick {tick}"
    );
    assert_eq!(
        native.game.vars.strategy.view_float_cursor,
        retail.peek16(WORK_RAM | RETAIL_VIEW_FLOAT_CURSOR),
        "Training view-float cursor at tick {tick}"
    );
    assert_eq!(
        native.game.vars.strategy.view_float_y,
        retail.peek16(WORK_RAM | RETAIL_VIEW_FLOAT_Y) as i16,
        "Training view-float value at tick {tick}"
    );
    assert_eq!(
        native.game.vars.gameflags,
        retail.peek8(WORK_RAM | RETAIL_GAMEFLAGS),
        "Training game flags at tick {tick}"
    );
    assert_eq!(
        [native.frame().msg_count1, native.frame().msg_count2],
        [
            retail.peek8(WORK_RAM | RETAIL_MESSAGE_COUNT),
            retail.peek8(WORK_RAM | RETAIL_MESSAGE_FACE_PHASE),
        ],
        "Training message state at tick {tick}"
    );
    assert_eq!(
        [
            (native.frame().camera.x >> 16) as i16,
            (native.frame().camera.y >> 16) as i16,
            (native.frame().camera.z >> 16) as i16,
        ],
        [
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_X) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Y) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Z) as i16,
        ],
        "Training camera position at tick {tick}"
    );
    let retail_order = retail.active_object_slots();
    let native_order = native.game.objs.active_indices();
    assert_eq!(
        native_order, retail_order,
        "Training active order at tick {tick}"
    );
    let retail_objects = retail.object_snapshot();
    for slot in retail_order {
        let retail_object = retail_objects[usize::from(slot)];
        let native_object = native.game.objs.aliens[usize::from(slot)];
        assert_eq!(
            (
                native_object.worldx,
                native_object.worldy,
                native_object.worldz
            ),
            (
                retail_object.worldx,
                retail_object.worldy,
                retail_object.worldz
            ),
            "Training object position for slot {slot} at tick {tick}"
        );
        if slot >= STARTUP_ROLE_SLOTS {
            assert_eq!(
                native_object.shape,
                retail_flat_shape(retail, retail_object.shape),
                "Training object shape for slot {slot} at tick {tick}"
            );
            let retail_base = RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride;
            assert_eq!(
                (
                    native_object.rotx,
                    native_object.roty,
                    native_object.rotz,
                    native_object.vel,
                ),
                (
                    retail.peek8(WORK_RAM | retail_base + AL_ROTX),
                    retail.peek8(WORK_RAM | retail_base + AL_ROTY),
                    retail.peek8(WORK_RAM | retail_base + AL_ROTZ),
                    retail.peek8(WORK_RAM | retail_base + AL_VEL),
                ),
                "Training object motion state for slot {slot} at tick {tick}"
            );
        }
        if slot == STARTUP_ROLE_SLOTS {
            let retail_base = RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride;
            assert_eq!(
                native_object.sbyte3,
                retail.peek8(WORK_RAM | retail_base + AL_SBYTE3),
                "Training controller path wait for slot {slot} at tick {tick}"
            );
            assert_eq!(
                native_object.sword2 as u16,
                retail.peek16(WORK_RAM | retail_base + AL_SWORD2),
                "Training controller path cursor for slot {slot} at tick {tick}"
            );
        }
    }
    assert_eq!(
        native.game.vars.rng,
        [
            retail.peek8(WORK_RAM | RETAIL_RAND),
            retail.peek8(WORK_RAM | RETAIL_RAND + 1),
            retail.peek8(WORK_RAM | RETAIL_RAND + 2),
            retail.peek8(WORK_RAM | RETAIL_RAND + 3),
        ],
        "Training random stream at tick {tick}; retail frame rate={}, native frame rate={}",
        retail.peek8(WORK_RAM | sf_oracle::RETAIL_FRAMERATE),
        native.game.vars.strategy.frame_rate,
    );
}

fn main() {
    let rom = load_retail_rom().expect("Star Fox retail ROM is required");
    let mut retail = RetailMachine::new(rom);
    let mut native = configured_shell();
    let mut retail_level_boundary_aligned = false;
    let mut certified_updates = 0u32;
    let mut previous_active = [false; sf_game::alien::NUMBER_AL];
    let mut observed_shapes = BTreeSet::new();
    let mut object_births = 0u32;
    let mut saw_final_pillar = false;
    let mut course_restarted = false;

    for tick in 0..=PROBE_END_TICK {
        let input = scripted_input(tick);
        let native_level_active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        if native_level_active {
            if !retail_level_boundary_aligned {
                assert!(
                    retail
                        .tick_until_cpu_execution(input, RETAIL_DOSTRATS, 240)
                        .expect("first Training entry"),
                    "retail did not reach the first Training entry"
                );
            }
            let max_video_frames = if tick <= 444 {
                240
            } else {
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
            };
            assert!(
                retail
                    .tick_until_cpu_execution(input, RETAIL_DOSTRATS, max_video_frames)
                    .expect("next Training update boundary"),
                "retail did not complete Training update {tick}"
            );
            retail_level_boundary_aligned = true;
        } else {
            retail
                .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
                .expect("retail Training front end");
        }
        native.tick(input);

        if native_level_active {
            assert_level_state(&retail, &native, tick);
            certified_updates += 1;

            let mut current_active = [false; sf_game::alien::NUMBER_AL];
            for slot in native.game.objs.active_indices() {
                let index = usize::from(slot);
                current_active[index] = true;
                if !previous_active[index] {
                    object_births += 1;
                    let object = native.game.objs.aliens[index];
                    observed_shapes.insert(object.shape);
                    if object.shape == sf_map::levels::route2::rc::SH_PILON && object.worldy <= -210
                    {
                        saw_final_pillar = true;
                    }
                    if saw_final_pillar && object.shape == sf_map::levels::route2::rc::SH_TRAINING {
                        course_restarted = true;
                    }
                }
            }
            previous_active = current_active;
        }
    }

    let missing_shapes = REQUIRED_TRAINING_SHAPES
        .iter()
        .filter_map(|(shape, name)| (!observed_shapes.contains(shape)).then_some(*name))
        .collect::<Vec<_>>();
    assert!(
        missing_shapes.is_empty(),
        "missing Training births: {missing_shapes:?}"
    );
    assert!(
        saw_final_pillar,
        "Training never reached the final pillar stretch"
    );
    assert!(
        course_restarted,
        "Training did not return to its main course loop"
    );
    assert_eq!(
        certified_updates,
        u32::from(native.game.vars.gameframe),
        "one certified update per Training game frame"
    );
    println!(
        "sf1_training_semantic certified_updates={certified_updates} first_divergence=none object_births={object_births} course_restarted={course_restarted} source_coverage={}",
        REQUIRED_TRAINING_SHAPES
            .iter()
            .map(|(_, name)| *name)
            .collect::<Vec<_>>()
            .join(",")
    );
}
