//! Strict retail/native trace for the complete first Training course cycle.

use sf_core::pad;
use sf_game::game::PosSndFamilyId;
use sf_game::shell::{GameState, GameplayEntryPhase, Shell, SoundCmd};
use sf_oracle::{
    load_retail_rom, RetailMachine, AL_AP, AL_COLLCOUNT, AL_COLLFLAGS, AL_HP, AL_ROTX, AL_ROTY,
    AL_ROTZ, AL_SBYTE3, AL_SFLAGS, AL_SFLAGS2, AL_SFLAGS3, AL_SWORD2, AL_VEL,
    RETAIL_BUILD_DRAWLIST_L, RETAIL_DOSTRATS, RETAIL_GAMEFLAGS, RETAIL_GAMEFRAME, RETAIL_LASTPLAYZ,
    RETAIL_LASTZCHANGE, RETAIL_MAPCNT, RETAIL_POOL, RETAIL_PVIEWPOSZ, RETAIL_PVIEWVELZ,
    RETAIL_RAND, RETAIL_SHAPES, RETAIL_SOUND_EFFECT_EVENTS, RETAIL_SOUND_EFFECT_WRITE_CURSOR,
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
const RETAIL_AL_SFLAGS4: u32 = AL_SFLAGS3 + 1;
const RETAIL_AL_COLLOBJPTR: u32 = 0x1B;
const RETAIL_AL_HIT_FLAGS: u32 = AL_COLLFLAGS + 7;
const GSU_DRAW_COUNT: usize = 0x01B6;
const GSU_DRAW_LIST: usize = 0x0EF2;
const DRAW_ENTRY_BYTES: usize = 30;
const DRAW_SORT_DEPTH: usize = 2;
const DRAW_ROTATION_X: usize = 4;
const DRAW_ROTATION_Y: usize = 5;
const DRAW_ROTATION_Z: usize = 6;
const DRAW_STRATEGY_FLAGS: usize = 7;
const DRAW_SHAPE: usize = 8;
const DRAW_POSITION_Y: usize = 16;
const DRAW_POSITION_X: usize = 18;
const DRAW_POSITION_Z: usize = 20;
const DRAW_COLOR_TABLE: usize = 22;
const DRAW_EXPLOSION_COUNT: usize = 24;
const DRAW_ANIMATION: usize = 25;
const DRAW_COLOR_FRAME: usize = 26;
const DRAW_DEPTH_OFFSET: usize = 27;
const DRAW_TEXTURE_SCROLL_X: usize = 28;
const DRAW_TEXTURE_SCROLL_Y: usize = 29;
const SOUND_EVENT_CAPACITY: u8 = 16;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position(i16, i16, i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TrainingDraw {
    list_order: usize,
    position: Position,
    rotation: [u8; 3],
    shape: u16,
    sort_depth: i16,
    strategy_flags: u8,
    color_table: u16,
    explosion_count: u8,
    animation: u8,
    color_frame: u8,
    depth_offset: u8,
    texture_scroll: [u8; 2],
}

fn configured_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    shell.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    shell.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    shell.set_prepare_presentation_player(Box::new(sf_strat::player::prepare_presentation_player));
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

fn gsu_word(machine: &RetailMachine, address: usize) -> u16 {
    u16::from_le_bytes([
        machine.peek_gsu_ram(address),
        machine.peek_gsu_ram(address + 1),
    ])
}

fn retail_draws(machine: &RetailMachine) -> Vec<TrainingDraw> {
    let count = usize::from(gsu_word(machine, GSU_DRAW_COUNT));
    (0..count)
        .map(|list_order| {
            let base = GSU_DRAW_LIST + list_order * DRAW_ENTRY_BYTES;
            TrainingDraw {
                list_order,
                position: Position(
                    gsu_word(machine, base + DRAW_POSITION_X) as i16,
                    gsu_word(machine, base + DRAW_POSITION_Y) as i16,
                    gsu_word(machine, base + DRAW_POSITION_Z) as i16,
                ),
                rotation: [
                    machine.peek_gsu_ram(base + DRAW_ROTATION_X),
                    machine.peek_gsu_ram(base + DRAW_ROTATION_Y),
                    machine.peek_gsu_ram(base + DRAW_ROTATION_Z),
                ],
                shape: retail_flat_shape(machine, gsu_word(machine, base + DRAW_SHAPE)),
                sort_depth: gsu_word(machine, base + DRAW_SORT_DEPTH) as i16,
                strategy_flags: machine.peek_gsu_ram(base + DRAW_STRATEGY_FLAGS),
                color_table: gsu_word(machine, base + DRAW_COLOR_TABLE),
                explosion_count: machine.peek_gsu_ram(base + DRAW_EXPLOSION_COUNT),
                animation: machine.peek_gsu_ram(base + DRAW_ANIMATION),
                color_frame: machine.peek_gsu_ram(base + DRAW_COLOR_FRAME),
                depth_offset: machine.peek_gsu_ram(base + DRAW_DEPTH_OFFSET),
                texture_scroll: [
                    machine.peek_gsu_ram(base + DRAW_TEXTURE_SCROLL_X),
                    machine.peek_gsu_ram(base + DRAW_TEXTURE_SCROLL_Y),
                ],
            }
        })
        .collect()
}

fn native_draws(shell: &Shell) -> Vec<TrainingDraw> {
    let camera = shell.frame().camera;
    let camera_position = Position(
        (camera.x >> 16) as i16,
        (camera.y >> 16) as i16,
        (camera.z >> 16) as i16,
    );
    let matrix = sf_core::snes_trig::zxy_matrix_q15_fine(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
    );
    shell
        .draw_list()
        .iter()
        .enumerate()
        .map(|(list_order, draw)| {
            let world = Position(
                (draw.x >> 16) as i16,
                (draw.y >> 16) as i16,
                (draw.z >> 16) as i16,
            );
            let relative = Position(
                world.0.wrapping_sub(camera_position.0),
                world.1.wrapping_sub(camera_position.1),
                world.2.wrapping_sub(camera_position.2),
            );
            let position =
                sf_core::snes_trig::matrix_rotate_q15(matrix, relative.0, relative.1, relative.2);
            let shape_sort_depth = sf_core::sf1_shape_metrics::sf1_shape_metrics(draw.shape_id)
                .map_or(0, |metrics| metrics.sort_depth);
            TrainingDraw {
                list_order,
                position: Position(position.0, position.1, position.2),
                rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                shape: draw.shape_id,
                sort_depth: draw
                    .sort_z
                    .wrapping_add(position.2)
                    .wrapping_add(shape_sort_depth),
                strategy_flags: draw.sflags,
                color_table: draw.color_table,
                explosion_count: draw.explosion_cnt,
                animation: draw.anim_frame,
                color_frame: draw.col_frame,
                depth_offset: draw.depth_offset,
                texture_scroll: [draw.tscroll_x, draw.tscroll_y],
            }
        })
        .collect()
}

fn retail_sound_events(machine: &RetailMachine, start: u8, end: u8) -> Vec<u8> {
    let mut cursor = start;
    let mut events = Vec::new();
    while cursor != end {
        events.push(machine.peek8(WORK_RAM | RETAIL_SOUND_EFFECT_EVENTS + u32::from(cursor)));
        cursor = (cursor + 1) % SOUND_EVENT_CAPACITY;
    }
    events
}

fn native_sound_events(shell: &mut Shell) -> Vec<u8> {
    let player_view_x = shell.frame().pviewposx;
    let player = shell.game.objs.player().copied();
    shell
        .drain_sound()
        .into_iter()
        .filter_map(|command| match command {
            SoundCmd::PlaySe(effect) => Some(effect),
            SoundCmd::MakeSnd { family, x, z } => player.and_then(|player| {
                sf_audio::sound::resolve_positional_effect(
                    player_view_x,
                    player.worldx,
                    player.worldz,
                    x,
                    z,
                    positional_sound_family(family),
                )
            }),
            _ => None,
        })
        .collect()
}

fn positional_sound_family(family: PosSndFamilyId) -> &'static sf_audio::sound::PosSndFamily {
    use sf_audio::sound::*;
    match family {
        PosSndFamilyId::Laser => &POS_LASER,
        PosSndFamilyId::Missile => &POS_MISSILE,
        PosSndFamilyId::HitWall => &POS_HITWALL,
        PosSndFamilyId::MoveWall => &POS_MOVEWALL,
        PosSndFamilyId::RingLaser => &POS_RINGLASER,
        PosSndFamilyId::DoorOpen => &POS_DOOROPEN,
        PosSndFamilyId::DoorClose => &POS_DOORCLOSE,
        PosSndFamilyId::EnemyUpSea => &POS_ENEMYUPSEA,
        PosSndFamilyId::EnemyDownSea => &POS_ENEMYDOWNSEA,
        PosSndFamilyId::DestBoss => &POS_DESTBOSS,
        PosSndFamilyId::DestEnemy => &POS_DESTENEMY,
        PosSndFamilyId::DamEnemy => &POS_DAMENEMY,
        PosSndFamilyId::EnemyBattry => &POS_ENEMYBATTRY,
        PosSndFamilyId::SeparateMissile => &POS_SEPARATEMISSILE,
    }
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
        let retail_base = RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride;
        assert_eq!(
            [
                native_object.sflags,
                native_object.sflags2,
                native_object.sflags3,
                native_object.sflags4,
            ],
            [
                retail.peek8(WORK_RAM | retail_base + AL_SFLAGS),
                retail.peek8(WORK_RAM | retail_base + AL_SFLAGS2),
                retail.peek8(WORK_RAM | retail_base + AL_SFLAGS3),
                retail.peek8(WORK_RAM | retail_base + RETAIL_AL_SFLAGS4),
            ],
            "Training object strategy flags for slot {slot} at tick {tick}"
        );
        assert_eq!(
            (
                native_object.hp,
                native_object.ap,
                native_object.collcount,
                native_object.collflags,
                native_object.hitflags,
            ),
            (
                retail.peek8(WORK_RAM | retail_base + AL_HP),
                retail.peek8(WORK_RAM | retail_base + AL_AP),
                retail.peek8(WORK_RAM | retail_base + AL_COLLCOUNT),
                retail.peek8(WORK_RAM | retail_base + AL_COLLFLAGS),
                retail.peek8(WORK_RAM | retail_base + RETAIL_AL_HIT_FLAGS),
            ),
            "Training object collision state for slot {slot} at tick {tick}: native shape={} strat={:?} collobj={} pos=({}, {}, {}), retail shape={} collobj={} pos=({}, {}, {})",
            native_object.shape,
            native_object.stratptr,
            native_object.collobjptr,
            native_object.worldx,
            native_object.worldy,
            native_object.worldz,
            retail_flat_shape(retail, retail_object.shape),
            retail.peek16(WORK_RAM | retail_base + RETAIL_AL_COLLOBJPTR),
            retail_object.worldx,
            retail_object.worldy,
            retail_object.worldz,
        );
        if slot == STARTUP_ROLE_SLOTS {
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
    let mut certified_draw_updates = 0u32;
    let mut certified_audio_updates = 0u32;

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
            let retail_sound_cursor = retail.peek8(WORK_RAM | RETAIL_SOUND_EFFECT_WRITE_CURSOR);
            assert!(
                retail
                    .tick_until_cpu_execution(input, RETAIL_BUILD_DRAWLIST_L, max_video_frames)
                    .expect("Training draw boundary"),
                "retail did not complete Training draw update {tick}"
            );
            let expected_draws = retail_draws(&retail);
            assert!(
                retail
                    .tick_until_cpu_execution(
                        scripted_input(tick.saturating_add(1)),
                        RETAIL_DOSTRATS,
                        max_video_frames,
                    )
                    .expect("next Training update boundary"),
                "retail did not reach the next Training update {tick}"
            );
            let next_retail_sound_cursor =
                retail.peek8(WORK_RAM | RETAIL_SOUND_EFFECT_WRITE_CURSOR);
            let expected_sounds =
                retail_sound_events(&retail, retail_sound_cursor, next_retail_sound_cursor);
            retail_level_boundary_aligned = true;

            native.tick(input);
            assert_eq!(
                native_draws(&native),
                expected_draws,
                "Training ordered draw commands at tick {tick}"
            );
            certified_draw_updates += 1;
            let actual_sounds = native_sound_events(&mut native);
            assert_eq!(
                actual_sounds, expected_sounds,
                "Training sound events at tick {tick}"
            );
            certified_audio_updates += 1;
        } else {
            retail
                .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
                .expect("retail Training front end");
            native.tick(input);
            native.drain_sound();
        }

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
    assert_eq!(
        certified_draw_updates, certified_updates,
        "one certified draw list per Training update"
    );
    assert_eq!(
        certified_audio_updates, certified_updates,
        "one certified sound stream per Training update"
    );
    println!(
        "sf1_training semantic_updates={certified_updates} draw_updates={certified_draw_updates} audio_updates={certified_audio_updates} first_divergence=none object_births={object_births} course_restarted={course_restarted} source_coverage={}",
        REQUIRED_TRAINING_SHAPES
            .iter()
            .map(|(_, name)| *name)
            .collect::<Vec<_>>()
            .join(",")
    );
}
