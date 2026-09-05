//! Strict retail/native trace for the complete first Training course cycle.

mod support;

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
use sf_render::renderer::{config_from_repo_root, Renderer};
use std::collections::BTreeSet;
use std::path::Path;

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
const GSU_DRAW_HEAD: usize = 0x021E;
const GSU_DRAW_LIST: usize = 0x0EF2;
const GSU_PROJECTION_COMPLETE_BANK: u8 = 1;
const GSU_PROJECTION_COMPLETE_INSTRUCTION: u16 = 0x8B57;
const GSU_CAPTURE_LENGTH: usize = 0x1200;
const GSU_FACE_POINTER: usize = 0x0018;
const GSU_SHAPE_POINTER: usize = 0x001A;
const GSU_VIEW_POSITION_X: usize = 0x0026;
const GSU_VIEW_POSITION_Y: usize = 0x0028;
const GSU_VIEW_POSITION_Z: usize = 0x002A;
const GSU_SHAPE_MATRIX: usize = 0x0120;
const GSU_OBJECT_LIGHT_X: usize = 0x00F4;
const GSU_OBJECT_LIGHT_Y: usize = 0x00F6;
const GSU_OBJECT_LIGHT_Z: usize = 0x00F8;
const GSU_POINT_COUNT: usize = 0x0132;
const GSU_FACE_POINT_COUNT: usize = 0x0134;
const GSU_ROTATED_POINTS: usize = 0x05C2;
const GSU_PROJECTED_POINTS: usize = 0x07A2;
const GSU_POLYGON_POINTS: usize = 0x0982;
const GSU_VISIBILITY: usize = 0x0E72;
const GSU_OBJECT_DEPTH: usize = 0x002A;
const GSU_COLOR_TABLE_POINTER: usize = 0x004A;
const GSU_SHADE_TABLE_POINTER: usize = 0x004C;
const GSU_DEPTH_COLOR_TABLE_POINTER: usize = 0x004E;
const GSU_DEPTH_THRESHOLD_TABLE_POINTER: usize = 0x0050;
const GSU_ACTIVE_DEPTH_COLOR_POINTER: usize = 0x0052;
const GSU_TEXTURE_COLOR_MODE: usize = 0x0058;
const GSU_SPRITE_BANK: usize = 0x0094;
const GSU_SPRITE_DATA: usize = 0x0096;
const GSU_SPRITE_MASK: usize = 0x00C2;
const GSU_DEPTH_OFFSET: usize = 0x0178;
const GSU_EXPLOSION_STATE: usize = 0x0048;
const GSU_EXPLODED_POSITION_X: usize = 0x0062;
const GSU_EXPLODED_POSITION_Y: usize = 0x002C;
const GSU_EXPLODED_POSITION_Z: usize = 0x002E;
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
const SOURCE_BITMAP_COLOR_DEPTH: usize = 4;
const SOURCE_BITMAP_TILE_WIDTH: usize = 8;
const SOURCE_BITMAP_TILE_BYTES: usize = SOURCE_BITMAP_COLOR_DEPTH * SOURCE_BITMAP_TILE_WIDTH;
const SOURCE_BITMAP_BASE_UNIT_BYTES: usize = 1_024;
const SOURCE_BITMAP_LAYOUT_HEIGHT_192: usize = 2;
const SOURCE_BITMAP_MODE_4BPP: u8 = 1;
const SOURCE_BITMAP_WIDTH: usize = 224;
const SOURCE_BITMAP_LEFT: usize = 16;
const SOURCE_BITMAP_TOP: usize = 16;
const FIRST_VISIBLE_SOURCE_BITMAP_GAME_FRAME: u16 = 7;
// Radio portraits and text begin at local row 152. Those bitmap-resident HUD
// primitives have their own strict compositor path; this direct raster gate
// certifies the uninterrupted 3D and point-field region above them.
const SOURCE_SCENE_CERTIFIED_HEIGHT: usize = 152;
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
    (0xADD5, sf_render::shape_data::SHAPE_EXT_SMOKE),
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

fn captured_word(capture: &[u8], address: usize) -> u16 {
    u16::from_le_bytes([capture[address], capture[address + 1]])
}

fn describe_projection_capture(capture: &sf_oracle::gsu::ExecutionCapture) -> String {
    let memory = &capture.memory;
    let point_count = usize::from(captured_word(memory, GSU_POINT_COUNT));
    let points = (0..point_count)
        .map(|index| {
            let address = GSU_PROJECTED_POINTS + index * 6;
            [
                captured_word(memory, address) as i16,
                captured_word(memory, address + 2) as i16,
                captured_word(memory, address + 4) as i16,
            ]
        })
        .collect::<Vec<_>>();
    let rotated = (0..point_count)
        .map(|index| {
            let address = GSU_ROTATED_POINTS + index * 6;
            [
                captured_word(memory, address) as i16,
                captured_word(memory, address + 2) as i16,
                captured_word(memory, address + 4) as i16,
            ]
        })
        .collect::<Vec<_>>();
    let matrix = (0..9)
        .map(|index| captured_word(memory, GSU_SHAPE_MATRIX + index * 2) as i16)
        .collect::<Vec<_>>();
    format!(
        "shape_pointer={:#06X} position=[{},{},{}] light=[{},{},{}] matrix={matrix:?} points={points:?} rotated={rotated:?}",
        captured_word(memory, GSU_SHAPE_POINTER),
        captured_word(memory, GSU_VIEW_POSITION_X) as i16,
        captured_word(memory, GSU_VIEW_POSITION_Y) as i16,
        captured_word(memory, GSU_VIEW_POSITION_Z) as i16,
        captured_word(memory, GSU_OBJECT_LIGHT_X) as i16,
        captured_word(memory, GSU_OBJECT_LIGHT_Y) as i16,
        captured_word(memory, GSU_OBJECT_LIGHT_Z) as i16,
    )
}

fn describe_pixel_writer(capture: &sf_oracle::gsu::ExecutionCapture) -> String {
    let point_count = usize::from(captured_word(&capture.memory, GSU_FACE_POINT_COUNT));
    let points = (0..point_count)
        .map(|index| {
            let address = GSU_POLYGON_POINTS + index * 4;
            [
                captured_word(&capture.memory, address) as i16,
                captured_word(&capture.memory, address + 2) as i16,
            ]
        })
        .collect::<Vec<_>>();
    let visibility = (0..6)
        .map(|index| capture.memory[GSU_VISIBILITY + index] as i8)
        .collect::<Vec<_>>();
    format!(
        "points={points:?} visibility={visibility:?} object_depth={} explosion={} exploded_position=[{},{},{}] color_table={:#06X} shade_table={:#06X} depth_colors={:#06X} depth_thresholds={:#06X} active_depth_colors={:#06X} depth_offset={} texture_color_mode={:#06X} sprite_bank={:#06X} sprite_data={:#06X} sprite_mask={:#06X}",
        captured_word(&capture.memory, GSU_OBJECT_DEPTH) as i16,
        captured_word(&capture.memory, GSU_EXPLOSION_STATE),
        captured_word(&capture.memory, GSU_EXPLODED_POSITION_X) as i16,
        captured_word(&capture.memory, GSU_EXPLODED_POSITION_Y) as i16,
        captured_word(&capture.memory, GSU_EXPLODED_POSITION_Z) as i16,
        captured_word(&capture.memory, GSU_COLOR_TABLE_POINTER),
        captured_word(&capture.memory, GSU_SHADE_TABLE_POINTER),
        captured_word(&capture.memory, GSU_DEPTH_COLOR_TABLE_POINTER),
        captured_word(&capture.memory, GSU_DEPTH_THRESHOLD_TABLE_POINTER),
        captured_word(&capture.memory, GSU_ACTIVE_DEPTH_COLOR_POINTER),
        captured_word(&capture.memory, GSU_DEPTH_OFFSET),
        captured_word(&capture.memory, GSU_TEXTURE_COLOR_MODE),
        captured_word(&capture.memory, GSU_SPRITE_BANK),
        captured_word(&capture.memory, GSU_SPRITE_DATA),
        captured_word(&capture.memory, GSU_SPRITE_MASK),
    )
}

fn retail_bitmap_pixel(machine: &RetailMachine, x: u8, y: u8) -> u8 {
    let (screen_base, screen_mode, ..) = machine.gsu_screen_state().expect("retail GSU state");
    assert_eq!(
        screen_mode & 3,
        SOURCE_BITMAP_MODE_4BPP,
        "Training source bitmap color depth"
    );
    let layout = usize::from((screen_mode >> 5) & 1) * 2 + usize::from((screen_mode >> 2) & 1);
    assert_eq!(
        layout, SOURCE_BITMAP_LAYOUT_HEIGHT_192,
        "Training source bitmap height"
    );
    let x = usize::from(x);
    let y = usize::from(y);
    let character = ((x & !7) << 1) + (x & !7) + ((y & !7) >> 3);
    let base = character * SOURCE_BITMAP_TILE_BYTES
        + usize::from(screen_base) * SOURCE_BITMAP_BASE_UNIT_BYTES
        + (y & 7) * 2;
    let bit = 7 - (x & 7);
    (0..SOURCE_BITMAP_COLOR_DEPTH).fold(0, |color, plane| {
        let plane_offset = (plane >> 1) * 16 + (plane & 1);
        color | (((machine.peek_gsu_ram((base + plane_offset) & 0xFFFF) >> bit) & 1) << plane)
    })
}

fn retail_draws(machine: &RetailMachine) -> Vec<TrainingDraw> {
    let count = usize::from(gsu_word(machine, GSU_DRAW_COUNT));
    let list_end = GSU_DRAW_LIST + count * DRAW_ENTRY_BYTES;
    let mut address = usize::from(gsu_word(machine, GSU_DRAW_HEAD));
    let mut draws = Vec::with_capacity(count);
    let mut visited = std::collections::HashSet::with_capacity(count);
    while address != 0 {
        assert!(
            (GSU_DRAW_LIST..list_end).contains(&address),
            "Training draw link {address:#06X} outside allocated list"
        );
        assert_eq!(
            (address - GSU_DRAW_LIST) % DRAW_ENTRY_BYTES,
            0,
            "unaligned Training draw link"
        );
        assert!(visited.insert(address), "cyclic Training draw chain");
        let list_order = draws.len();
        draws.push(TrainingDraw {
            list_order,
            position: Position(
                gsu_word(machine, address + DRAW_POSITION_X) as i16,
                gsu_word(machine, address + DRAW_POSITION_Y) as i16,
                gsu_word(machine, address + DRAW_POSITION_Z) as i16,
            ),
            rotation: [
                machine.peek_gsu_ram(address + DRAW_ROTATION_X),
                machine.peek_gsu_ram(address + DRAW_ROTATION_Y),
                machine.peek_gsu_ram(address + DRAW_ROTATION_Z),
            ],
            shape: retail_flat_shape(machine, gsu_word(machine, address + DRAW_SHAPE)),
            sort_depth: gsu_word(machine, address + DRAW_SORT_DEPTH) as i16,
            strategy_flags: machine.peek_gsu_ram(address + DRAW_STRATEGY_FLAGS),
            color_table: gsu_word(machine, address + DRAW_COLOR_TABLE),
            explosion_count: machine.peek_gsu_ram(address + DRAW_EXPLOSION_COUNT),
            animation: machine.peek_gsu_ram(address + DRAW_ANIMATION),
            color_frame: machine.peek_gsu_ram(address + DRAW_COLOR_FRAME),
            depth_offset: machine.peek_gsu_ram(address + DRAW_DEPTH_OFFSET),
            texture_scroll: [
                machine.peek_gsu_ram(address + DRAW_TEXTURE_SCROLL_X),
                machine.peek_gsu_ram(address + DRAW_TEXTURE_SCROLL_Y),
            ],
        });
        address = usize::from(gsu_word(machine, address));
    }
    assert_eq!(draws.len(), count, "Training draw chain length");
    draws
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
    let mut draws = shell
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
        .collect::<Vec<_>>();
    draws.sort_by_key(|draw| {
        (
            std::cmp::Reverse(draw.sort_depth),
            std::cmp::Reverse(draw.list_order),
        )
    });
    for (list_order, draw) in draws.iter_mut().enumerate() {
        draw.list_order = list_order;
    }
    draws
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
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(256, 224, &config_from_repo_root(repo_root))
        .expect("headless Training source renderer");
    let mut retail_level_boundary_aligned = false;
    let mut certified_updates = 0u32;
    let mut previous_active = [false; sf_game::alien::NUMBER_AL];
    let mut observed_shapes = BTreeSet::new();
    let mut object_births = 0u32;
    let mut saw_final_pillar = false;
    let mut course_restarted = false;
    let mut certified_draw_updates = 0u32;
    let mut certified_audio_updates = 0u32;
    let mut certified_bitmap_updates = 0u32;
    let bitmap_dump_frame = std::env::var("SF1_TRAINING_TRACE_BITMAP_DUMP_FRAME")
        .ok()
        .map(|value| value.parse::<u16>().expect("Training bitmap dump frame"));
    let bitmap_dump_directory =
        std::env::var_os("SF1_TRAINING_TRACE_BITMAP_DUMP_DIR").map(std::path::PathBuf::from);
    let projection_probe_frame = std::env::var("SF1_TRAINING_TRACE_PROJECTION_FRAME")
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("Training projection probe frame")
        });
    let pixel_probe = std::env::var("SF1_TRAINING_TRACE_PIXEL").ok().map(|value| {
        let (x, y) = value
            .split_once(',')
            .expect("Training pixel probe must be x,y");
        [
            x.parse::<u8>().expect("Training pixel probe x"),
            y.parse::<u8>().expect("Training pixel probe y"),
        ]
    });
    let mut previous_point_pixels = Vec::new();

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
            let capture_projection =
                projection_probe_frame == Some(native.game.vars.gameframe.wrapping_add(1));
            if capture_projection {
                for draw in &expected_draws {
                    println!(
                        "Training retail draw game_frame={} object={} shape={} color_table={} depth_offset={} texture_scroll={:?}",
                        projection_probe_frame.expect("projection probe frame"),
                        draw.list_order,
                        draw.shape,
                        draw.color_table,
                        draw.depth_offset,
                        draw.texture_scroll,
                    );
                }
                retail.watch_gsu_execution_capture(
                    GSU_PROJECTION_COMPLETE_BANK,
                    GSU_PROJECTION_COMPLETE_INSTRUCTION,
                    0,
                    GSU_CAPTURE_LENGTH,
                );
                if let Some([x, y]) = pixel_probe {
                    retail.watch_gsu_pixel_writes(x, y);
                }
            }
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
            if capture_projection {
                for (sequence, capture) in retail.take_gsu_execution_captures().iter().enumerate() {
                    println!(
                        "Training retail projection game_frame={} sequence={sequence} {}",
                        projection_probe_frame.expect("projection probe frame"),
                        describe_projection_capture(capture),
                    );
                }
                for (sequence, capture) in retail.take_gsu_pixel_write_captures().iter().enumerate()
                {
                    println!(
                        "Training retail pixel writer game_frame={} sequence={sequence} instruction={:#06X} shape_pointer={:#06X} face_pointer={:#06X} color={} {} values={:?}",
                        projection_probe_frame.expect("projection probe frame"),
                        capture.instruction,
                        captured_word(&capture.memory, GSU_SHAPE_POINTER),
                        captured_word(&capture.memory, GSU_FACE_POINTER),
                        capture.color,
                        describe_pixel_writer(capture),
                        capture.values,
                    );
                }
            }
            let next_retail_sound_cursor =
                retail.peek8(WORK_RAM | RETAIL_SOUND_EFFECT_WRITE_CURSOR);
            let expected_sounds =
                retail_sound_events(&retail, retail_sound_cursor, next_retail_sound_cursor);
            retail_level_boundary_aligned = true;

            if projection_probe_frame == Some(native.game.vars.gameframe.wrapping_add(1)) {
                previous_point_pixels = native.frame().point_pixels;
            }
            native.tick(input);
            if projection_probe_frame == Some(native.game.vars.gameframe) {
                println!(
                    "Training native camera game_frame={} position={:?} rotation={:?}",
                    native.game.vars.gameframe,
                    [
                        (native.frame().camera.x >> 16) as i16,
                        (native.frame().camera.y >> 16) as i16,
                        (native.frame().camera.z >> 16) as i16,
                    ],
                    native.frame().camera.rotation,
                );
                let diagnostic_frame = native.frame();
                println!(
                    "Training native point field game_frame={} count={} corner={:?} previous_corner={:?}",
                    native.game.vars.gameframe,
                    diagnostic_frame.point_pixels.len(),
                    diagnostic_frame
                        .point_pixels
                        .iter()
                        .filter(|point| point.x <= 1 && point.y <= 1)
                        .collect::<Vec<_>>(),
                    previous_point_pixels
                        .iter()
                        .filter(|point| point.x <= 1 && point.y <= 1)
                        .collect::<Vec<_>>(),
                );
                for (list_order, draw) in native.draw_list().iter().enumerate() {
                    println!(
                        "Training native draw game_frame={} object={} owner={} shape={} flags={} strategy_flags={} color_table={} animation_frame={} color_frame={} depth_offset={} explosion={} position={:?} rotation={:?} shadow_position={:?}",
                        native.game.vars.gameframe,
                        list_order,
                        draw.obj_id,
                        sf_render::shapes::resolve_shape_word(draw.shape_id),
                        draw.flags,
                        draw.sflags,
                        draw.color_table,
                        draw.anim_frame,
                        draw.col_frame,
                        draw.depth_offset,
                        draw.explosion_cnt,
                        [(draw.x >> 16) as i16, (draw.y >> 16) as i16, (draw.z >> 16) as i16],
                        [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                        [draw.shad_x, draw.shad_y, draw.shad_z],
                    );
                }
                for projection in support::native_source_projections(&native) {
                    println!(
                        "Training native projection game_frame={} object={} shape={} position={:?} light={:?} points={:?}",
                        native.game.vars.gameframe,
                        projection.list_order,
                        projection.shape,
                        projection.position,
                        projection.object_light,
                        projection.points,
                    );
                }
                for projection in support::native_source_shadow_projections(&native) {
                    println!(
                        "Training native shadow projection game_frame={} object={} shape={} position={:?} points={:?}",
                        native.game.vars.gameframe,
                        projection.list_order,
                        projection.shape,
                        projection.position,
                        projection.points,
                    );
                }
                for projection in support::native_source_exploded_shadow_faces(&native) {
                    println!(
                        "Training native exploded shadow face game_frame={} face={} shape={} position={:?} points={:?}",
                        native.game.vars.gameframe,
                        projection.list_order,
                        projection.shape,
                        projection.position,
                        projection.points,
                    );
                }
            }
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

            if native.game.vars.gameframe >= FIRST_VISIBLE_SOURCE_BITMAP_GAME_FRAME {
                let native_draw_list = native
                    .draw_list()
                    .iter()
                    .map(support::render_entry)
                    .collect::<Vec<_>>();
                support::render_playing_snapshot(
                    &native.frame(),
                    &[],
                    &native_draw_list,
                    &mut renderer,
                );
                let native_bitmap = renderer.source_bitmap_indices();
                if bitmap_dump_frame == Some(native.game.vars.gameframe) {
                    let directory = bitmap_dump_directory
                        .as_ref()
                        .expect("Training bitmap dump frame requires a directory");
                    std::fs::create_dir_all(directory)
                        .expect("create Training bitmap dump directory");
                    std::fs::write(
                        directory.join(format!(
                            "native-bitmap-{:04}.bin",
                            native.game.vars.gameframe
                        )),
                        native_bitmap,
                    )
                    .expect("write Training native bitmap");
                    let mut retail_bitmap =
                        Vec::with_capacity(SOURCE_SCENE_CERTIFIED_HEIGHT * SOURCE_BITMAP_WIDTH);
                    for y in 0..SOURCE_SCENE_CERTIFIED_HEIGHT {
                        for x in 0..SOURCE_BITMAP_WIDTH {
                            retail_bitmap.push(retail_bitmap_pixel(
                                &retail,
                                u8::try_from(x).expect("Training bitmap x"),
                                u8::try_from(y).expect("Training bitmap y"),
                            ));
                        }
                    }
                    std::fs::write(
                        directory.join(format!(
                            "retail-bitmap-{:04}.bin",
                            native.game.vars.gameframe
                        )),
                        retail_bitmap,
                    )
                    .expect("write Training retail bitmap");
                    std::fs::write(
                        directory.join(format!(
                            "native-owners-{:04}.bin",
                            native.game.vars.gameframe
                        )),
                        renderer
                            .source_bitmap_owners()
                            .iter()
                            .flat_map(|owner| owner.to_le_bytes())
                            .collect::<Vec<_>>(),
                    )
                    .expect("write Training native bitmap owners");
                    std::fs::write(
                        directory.join(format!(
                            "native-faces-{:04}.bin",
                            native.game.vars.gameframe
                        )),
                        renderer
                            .source_bitmap_faces()
                            .iter()
                            .flat_map(|face| face.to_le_bytes())
                            .collect::<Vec<_>>(),
                    )
                    .expect("write Training native bitmap faces");
                }
                for y in 0..SOURCE_SCENE_CERTIFIED_HEIGHT {
                    for x in 0..SOURCE_BITMAP_WIDTH {
                        let retail_index = retail_bitmap_pixel(
                            &retail,
                            u8::try_from(x).expect("Training bitmap x"),
                            u8::try_from(y).expect("Training bitmap y"),
                        );
                        let native_index =
                            native_bitmap[(y + SOURCE_BITMAP_TOP) * 256 + x + SOURCE_BITMAP_LEFT];
                        assert_eq!(
                            native_index, retail_index,
                            "Training source bitmap at game frame {}, local pixel {x},{y}",
                            native.game.vars.gameframe,
                        );
                    }
                }
                certified_bitmap_updates += 1;
            }
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
    assert_eq!(
        certified_bitmap_updates,
        certified_updates - u32::from(FIRST_VISIBLE_SOURCE_BITMAP_GAME_FRAME - 1),
        "one certified source bitmap per visible Training update"
    );
    println!(
        "sf1_training semantic_updates={certified_updates} draw_updates={certified_draw_updates} bitmap_updates={certified_bitmap_updates} bitmap_pixels_per_update={} audio_updates={certified_audio_updates} first_divergence=none object_births={object_births} course_restarted={course_restarted} source_coverage={}",
        SOURCE_BITMAP_WIDTH * SOURCE_SCENE_CERTIFIED_HEIGHT,
        REQUIRED_TRAINING_SHAPES
            .iter()
            .map(|(_, name)| *name)
            .collect::<Vec<_>>()
            .join(",")
    );
}
