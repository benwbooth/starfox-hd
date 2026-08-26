//! Strict retail/native trace for the first Corneria player-laser burst.
//!
//! Source addresses and cartridge storage are confined to this oracle adapter;
//! the native side is read only through typed flat game objects.

mod support;

use sf_core::pad;
use sf_difftest::{
    compare_source_rgb, read_source_rgb_ppm, write_source_rgb_ppm, SourceVideoDivergence,
};
use sf_game::presentation::CompletedPresentationQueue;
use sf_game::shell::{GameState, GameplayEntryPhase, Shell, SoundCmd};
use sf_oracle::{
    load_retail_rom, CompletedRaster, RetailMachine, AL_AP, AL_COLLFLAGS, AL_HP, AL_IMMUNEPTR,
    AL_LIFECNT, AL_ROTX, AL_ROTY, AL_ROTZ, AL_SBYTE1, AL_SBYTE2, AL_SBYTE3, AL_TYPE, AL_VEL, AL_VX,
    AL_VY, AL_VZ, RETAIL_AL_ANIMFRAME, RETAIL_BG2SCROLL, RETAIL_BUILD_DRAWLIST_L, RETAIL_DOSTRATS,
    RETAIL_DOVOFS, RETAIL_FRAMERATE, RETAIL_GAMEFRAME, RETAIL_PLAYPT, RETAIL_PLROTZ, RETAIL_POOL,
    RETAIL_SOUND_EFFECT_EVENTS, RETAIL_SOUND_EFFECT_WRITE_CURSOR, RETAIL_VIEW_POSITION_X,
    RETAIL_VIEW_POSITION_Y, RETAIL_VIEW_POSITION_Z,
};
use sf_render::renderer::{config_from_repo_root, Renderer};
use std::collections::VecDeque;

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const COMPLETED_FRAME_ALIGNMENT_TICK: u32 = 900;
const CORNERIA_AUDIO_UPLOAD_TICK: u32 = 1_080;
const FIRST_LEVEL_STATE_TICK: u32 = 892;
const FIRE_START_TICK: u32 = 1_212;
const BANK_PROBE_INPUT_START_TICK: u32 = 1_188;
const BANK_PROBE_LAST_GAME_FRAME: u16 = 337;
const SOURCE_FRAME_WIDTH: usize = 256;
const SOURCE_FRAME_HEIGHT: usize = 224;
const PLAYER_LASER_SOUND: u8 = 53;
const SOURCE_PRESENTATION_RETENTION_PREROLL_GAME_FRAME: u16 = 190;
const SOURCE_VIDEO_FIRST_FRAME_ENV: &str = "SF1_SOURCE_VIDEO_FIRST_GAME_FRAME";
const SOURCE_VIDEO_LAST_FRAME_ENV: &str = "SF1_SOURCE_VIDEO_LAST_GAME_FRAME";
const SOUND_EVENT_CAPACITY: u8 = 16;
const SOURCE_PLAYER_LASER_SHAPE: u16 = 0xB369;
const NATIVE_PLAYER_LASER_SHAPE: u16 = 511;

const GSU_DRAW_COUNT: usize = 0x01B6;
const GSU_DRAW_LIST: usize = 0x0EF2;
const GSU_WORLD_MATRIX: usize = 0x00D2;
const GSU_GRID_PREPARATION: usize = 0x0142;
const GSU_PROJECTION_COMPLETE_BANK: u8 = 1;
const GSU_PROJECTION_COMPLETE_INSTRUCTION: u16 = 0x8B57;
const GSU_OBJECT_START_BANK: u8 = 1;
const GSU_OBJECT_START_INSTRUCTION: u16 = 0x8456;
const GSU_POLYGON_START_BANK: u8 = 1;
const GSU_POLYGON_START_INSTRUCTION: u16 = 0xA66B;
const GSU_SCANLINE_START_BANK: u8 = 1;
const GSU_SCANLINE_START_INSTRUCTION: u16 = 0xA752;
const GSU_REDUCED_SPRITE_PLOT_BANK: u8 = 1;
const GSU_REDUCED_SPRITE_PLOT_INSTRUCTION: u16 = 0xCDEE;
const GSU_CAPTURE_START: usize = 0;
const GSU_CAPTURE_LENGTH: usize = 0x1200;
const GSU_FACE_POINTER: usize = 0x0018;
const GSU_SHAPE_POINTER: usize = 0x001A;
const GSU_SHAPE_BANK: usize = 0x001C;
const GSU_VIEW_POSITION_X: usize = 0x0026;
const GSU_VIEW_POSITION_Y: usize = 0x0028;
const GSU_VIEW_POSITION_Z: usize = 0x002A;
const GSU_SHAPE_SCALE: usize = 0x0030;
const GSU_SHAPE_SHIFT: usize = 0x0032;
const GSU_VANISHING_POINT_X: usize = 0x0034;
const GSU_VANISHING_POINT_Y: usize = 0x0036;
const GSU_TEXTURE_MODE: usize = 0x0042;
const GSU_COLOR_FRAME: usize = 0x0046;
const GSU_COLOR_TABLE_POINTER: usize = 0x004A;
const GSU_TEXTURE_COLOR_MODE: usize = 0x0058;
const GSU_OBJECT_FLAGS: usize = 0x0054;
const GSU_SPRITE_BANK: usize = 0x0094;
const GSU_SPRITE_DATA: usize = 0x0096;
const GSU_SPRITE_AUTHORED_EXTENT: usize = 0x0098;
const GSU_SPRITE_LEFT_CLIP: usize = 0x00A0;
const GSU_SPRITE_RIGHT_CLIP: usize = 0x00A2;
const GSU_SPRITE_X: usize = 0x00A8;
const GSU_SPRITE_Y: usize = 0x00AA;
const GSU_SPRITE_SOURCE_SIZE: usize = 0x00B6;
const GSU_SPRITE_PROJECTED_SIZE: usize = 0x00BC;
const GSU_SPRITE_MASK: usize = 0x00C2;
const GSU_SPRITE_MATERIAL_INDEX: usize = 0x00C4;
const GSU_POINT_COUNT: usize = 0x0132;
const GSU_HORIZONTAL_SCROLL_OFFSET: usize = 0x0184;
// The retail build reserves 80 rotated points. The reconstructed build's
// symbol map reserves 250, so its later buffer address does not apply here.
const GSU_PROJECTED_POINTS: usize = 0x07A2;
const GSU_POLYGON_POINTS: usize = 0x0982;
const PROJECTION_COORDINATES_PER_POINT: usize = 2;
const POLYGON_BYTES_PER_POINT: usize = 4;
const SOURCE_BITMAP_COLOR_DEPTH: usize = 4;
const SOURCE_BITMAP_TILE_WIDTH: usize = 8;
const SOURCE_BITMAP_TILE_BYTES: usize = SOURCE_BITMAP_COLOR_DEPTH * SOURCE_BITMAP_TILE_WIDTH;
const SOURCE_BITMAP_BASE_UNIT_BYTES: usize = 1_024;
const SOURCE_BITMAP_LAYOUT_HEIGHT_192: usize = 2;
const SOURCE_BITMAP_MODE_4BPP: u8 = 1;
const SOURCE_BITMAP_INDEX_MASK: u8 = 15;
const SOURCE_BITMAP_WIDTH: usize = 224;
const SOURCE_BITMAP_HEIGHT: usize = 192;
const SOURCE_BITMAP_LEFT: usize = 16;
const SOURCE_BITMAP_TOP: usize = 16;
// Retail ALCS.INC places the 128-byte `bg2voffsbak` immediately before the
// known `bg2scroll` word. Source addresses stay confined to this oracle.
const RETAIL_BG2_VERTICAL_OFFSETS: u32 = RETAIL_BG2SCROLL - 128;
const RETAIL_BG2_VERTICAL_ENABLE: u16 = 1 << 14;
const TARGET_BUILDING_VIEW_POSITION: [i16; 3] = [-1_044, 142, 2_464];
const TARGET_DISPLAY_PIXEL: [usize; 2] = [16, 88];
const PRESENTATION_PROBE_VIDEO_FRAMES: u32 = 8;
const DRAW_ENTRY_BYTES: usize = 30;
const DRAW_ROTATION_X: usize = 4;
const DRAW_ROTATION_Y: usize = 5;
const DRAW_ROTATION_Z: usize = 6;
const DRAW_SHAPE: usize = 8;
const DRAW_POSITION_Y: usize = 16;
const DRAW_POSITION_X: usize = 18;
const DRAW_POSITION_Z: usize = 20;
const DRAW_ANIMATION: usize = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position(i16, i16, i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LaserState {
    slot: u16,
    list_order: usize,
    position: Position,
    rotation: [u8; 3],
    speed: u8,
    velocity: Position,
    lifetime: u8,
    hit_points: u8,
    attack_power: u8,
    object_type: u8,
    collision_flags: u8,
    aim: [u8; 2],
    owner_speed: u8,
    owner: u16,
    animation: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LaserDraw {
    list_order: usize,
    position: Position,
    rotation: [u8; 3],
    animation: u8,
}

struct PendingSourceVideo {
    game_frame: u16,
    native_rgb: Vec<u8>,
    retail_bitmap: Vec<u8>,
}

fn completed_raster_contains_bitmap(raster: &CompletedRaster, bitmap: &[u8]) -> bool {
    if raster.bg1_indices.len() != SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT
        || bitmap.len() != SOURCE_BITMAP_WIDTH * SOURCE_BITMAP_HEIGHT
    {
        return false;
    }
    (0..SOURCE_BITMAP_HEIGHT).all(|y| {
        (0..SOURCE_BITMAP_WIDTH).all(|x| {
            let source = bitmap[y * SOURCE_BITMAP_WIDTH + x];
            let completed = raster.bg1_indices
                [(y + SOURCE_BITMAP_TOP) * SOURCE_FRAME_WIDTH + x + SOURCE_BITMAP_LEFT];
            let completed = if completed == u8::MAX {
                0
            } else {
                completed & SOURCE_BITMAP_INDEX_MASK
            };
            source == completed
        })
    })
}

fn completed_raster_bitmap_differences(raster: &CompletedRaster, bitmap: &[u8]) -> Option<usize> {
    (raster.bg1_indices.len() == SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT
        && bitmap.len() == SOURCE_BITMAP_WIDTH * SOURCE_BITMAP_HEIGHT)
        .then(|| {
            (0..SOURCE_BITMAP_HEIGHT)
                .flat_map(|y| (0..SOURCE_BITMAP_WIDTH).map(move |x| (x, y)))
                .filter(|(x, y)| {
                    let source = bitmap[*y * SOURCE_BITMAP_WIDTH + *x];
                    let completed = raster.bg1_indices
                        [(*y + SOURCE_BITMAP_TOP) * SOURCE_FRAME_WIDTH + *x + SOURCE_BITMAP_LEFT];
                    let completed = if completed == u8::MAX {
                        0
                    } else {
                        completed & SOURCE_BITMAP_INDEX_MASK
                    };
                    source != completed
                })
                .count()
        })
}

fn completed_raster_rgb(raster: &CompletedRaster) -> Vec<u8> {
    assert_eq!(
        raster.rgba.len(),
        SOURCE_FRAME_WIDTH * SOURCE_FRAME_HEIGHT * 4,
        "completed retail raster dimensions"
    );
    raster
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
}

fn trace_input(tick: u32, bank_probe: bool) -> u16 {
    support::weapon_input(tick)
        | if bank_probe && tick >= BANK_PROBE_INPUT_START_TICK {
            pad::LEFT
        } else {
            0
        }
}

fn retail_object_base(slot: u16) -> u32 {
    RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride
}

fn source_rgb(frame: sf_oracle::PpuFrame) -> Vec<u8> {
    assert_eq!(frame.width, SOURCE_FRAME_WIDTH, "retail source frame width");
    assert_eq!(
        frame.height, SOURCE_FRAME_HEIGHT,
        "retail source frame height"
    );
    frame
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .collect()
}

fn retail_bg2_vertical_offsets(
    machine: &RetailMachine,
) -> Option<[i16; sf_core::scene::BG2_VERTICAL_OFFSET_COLUMNS]> {
    (machine.peek8(WORK_RAM | RETAIL_DOVOFS) != 0).then(|| {
        let scroll = machine.peek16(WORK_RAM | RETAIL_BG2SCROLL);
        std::array::from_fn(|column| {
            machine
                .peek16(
                    WORK_RAM
                        | (RETAIL_BG2_VERTICAL_OFFSETS
                            + u32::try_from(column * 2)
                                .expect("background offset address")),
                )
                .wrapping_sub(scroll)
                .wrapping_sub(RETAIL_BG2_VERTICAL_ENABLE) as i16
        })
    })
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
                captured_word(memory, address + PROJECTION_COORDINATES_PER_POINT) as i16,
            ]
        })
        .collect::<Vec<_>>();
    let outcodes = (0..point_count)
        .map(|index| captured_word(memory, GSU_PROJECTED_POINTS + index * 6 + 4))
        .collect::<Vec<_>>();
    format!(
        "face={:02X}:{:04X} view=[{},{},{}] scale={} shift={} object_flags={:04X} points={points:?} outcodes={outcodes:04X?}",
        captured_word(memory, GSU_SHAPE_BANK) as u8,
        captured_word(memory, GSU_FACE_POINTER),
        captured_word(memory, GSU_VIEW_POSITION_X) as i16,
        captured_word(memory, GSU_VIEW_POSITION_Y) as i16,
        captured_word(memory, GSU_VIEW_POSITION_Z) as i16,
        captured_word(memory, GSU_SHAPE_SCALE),
        captured_word(memory, GSU_SHAPE_SHIFT),
        captured_word(memory, GSU_OBJECT_FLAGS),
    )
}

fn projection_capture_position(capture: &sf_oracle::gsu::ExecutionCapture) -> [i16; 3] {
    [
        captured_word(&capture.memory, GSU_VIEW_POSITION_X) as i16,
        captured_word(&capture.memory, GSU_VIEW_POSITION_Y) as i16,
        captured_word(&capture.memory, GSU_VIEW_POSITION_Z) as i16,
    ]
}

fn describe_polygon_capture(capture: &sf_oracle::gsu::ExecutionCapture) -> String {
    let point_count = usize::from(capture.values[0]);
    let points = (0..point_count)
        .map(|index| {
            let address = GSU_POLYGON_POINTS + index * POLYGON_BYTES_PER_POINT;
            [
                captured_word(&capture.memory, address) as i16,
                captured_word(&capture.memory, address + 2) as i16,
            ]
        })
        .collect::<Vec<_>>();
    format!(
        "shape_pointer={:04X} color={} points={points:?}",
        captured_word(&capture.memory, GSU_SHAPE_POINTER),
        capture.color,
    )
}

fn retail_slot_from_pointer(pointer: u16) -> u16 {
    let relative = u32::from(pointer)
        .checked_sub(RETAIL_POOL.base)
        .expect("object pointer precedes retail pool");
    assert_eq!(relative % RETAIL_POOL.stride, 0, "unaligned object pointer");
    u16::try_from(relative / RETAIL_POOL.stride).expect("object slot")
}

fn retail_lasers(machine: &RetailMachine) -> Vec<LaserState> {
    let player_pointer = machine.peek16(WORK_RAM | RETAIL_PLAYPT);
    machine
        .active_object_slots()
        .into_iter()
        .enumerate()
        .filter_map(|(list_order, slot)| {
            let base = retail_object_base(slot);
            (machine.peek16(WORK_RAM | base + RETAIL_POOL.al_shape) == SOURCE_PLAYER_LASER_SHAPE)
                .then(|| LaserState {
                    slot,
                    list_order,
                    position: Position(
                        machine.peek16(WORK_RAM | base + RETAIL_POOL.al_worldx) as i16,
                        machine.peek16(WORK_RAM | base + RETAIL_POOL.al_worldy) as i16,
                        machine.peek16(WORK_RAM | base + RETAIL_POOL.al_worldz) as i16,
                    ),
                    rotation: [
                        machine.peek8(WORK_RAM | base + AL_ROTX),
                        machine.peek8(WORK_RAM | base + AL_ROTY),
                        machine.peek8(WORK_RAM | base + AL_ROTZ),
                    ],
                    speed: machine.peek8(WORK_RAM | base + AL_VEL),
                    velocity: Position(
                        machine.peek16(WORK_RAM | base + AL_VX) as i16,
                        machine.peek16(WORK_RAM | base + AL_VY) as i16,
                        machine.peek16(WORK_RAM | base + AL_VZ) as i16,
                    ),
                    lifetime: machine.peek8(WORK_RAM | base + AL_LIFECNT),
                    hit_points: machine.peek8(WORK_RAM | base + AL_HP),
                    attack_power: machine.peek8(WORK_RAM | base + AL_AP),
                    object_type: machine.peek8(WORK_RAM | base + AL_TYPE),
                    collision_flags: machine.peek8(WORK_RAM | base + AL_COLLFLAGS),
                    aim: [
                        machine.peek8(WORK_RAM | base + AL_SBYTE1),
                        machine.peek8(WORK_RAM | base + AL_SBYTE2),
                    ],
                    owner_speed: machine.peek8(WORK_RAM | base + AL_SBYTE3),
                    owner: retail_slot_from_pointer(machine.peek16(WORK_RAM | base + AL_IMMUNEPTR)),
                    animation: machine.peek8(WORK_RAM | RETAIL_AL_ANIMFRAME + base),
                })
        })
        .inspect(|laser| {
            assert_eq!(
                laser.owner,
                retail_slot_from_pointer(player_pointer),
                "retail laser owner"
            );
        })
        .collect()
}

fn native_lasers(shell: &Shell) -> Vec<LaserState> {
    shell
        .game
        .objs
        .active_indices()
        .into_iter()
        .enumerate()
        .filter_map(|(list_order, slot)| {
            let object = shell.game.objs.aliens[usize::from(slot)];
            (object.shape == NATIVE_PLAYER_LASER_SHAPE).then_some(LaserState {
                slot,
                list_order,
                position: Position(object.worldx, object.worldy, object.worldz),
                rotation: [object.rotx, object.roty, object.rotz],
                speed: object.vel,
                velocity: Position(object.vx, object.vy, object.vz),
                lifetime: object.count,
                hit_points: object.hp,
                attack_power: object.ap,
                object_type: object.type_,
                collision_flags: object.collflags,
                aim: [object.sbyte1, object.sbyte2],
                owner_speed: object.sbyte3,
                owner: object.immuneptr,
                animation: object.animframe,
            })
        })
        .collect()
}

fn gsu_word(machine: &RetailMachine, address: usize) -> u16 {
    u16::from_le_bytes([
        machine.peek_gsu_ram(address),
        machine.peek_gsu_ram(address + 1),
    ])
}

fn retail_bitmap_pixel(machine: &RetailMachine, x: u8, y: u8) -> u8 {
    let (screen_base, screen_mode, ..) = machine.gsu_screen_state().expect("retail GSU state");
    assert_eq!(
        screen_mode & 3,
        SOURCE_BITMAP_MODE_4BPP,
        "weapon capture source bitmap color depth"
    );
    let layout = usize::from((screen_mode >> 5) & 1) * 2
        + usize::from((screen_mode >> 2) & 1);
    assert_eq!(
        layout, SOURCE_BITMAP_LAYOUT_HEIGHT_192,
        "weapon capture source bitmap height"
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

fn retail_bitmap_indices(machine: &RetailMachine) -> Vec<u8> {
    (0..SOURCE_BITMAP_HEIGHT)
        .flat_map(|y| {
            (0..SOURCE_BITMAP_WIDTH).map(move |x| {
                retail_bitmap_pixel(
                    machine,
                    u8::try_from(x).expect("source bitmap x"),
                    u8::try_from(y).expect("source bitmap y"),
                )
            })
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

fn retail_laser_draws(machine: &RetailMachine) -> Vec<LaserDraw> {
    let count = usize::from(gsu_word(machine, GSU_DRAW_COUNT));
    (0..count)
        .filter_map(|list_order| {
            let base = GSU_DRAW_LIST + list_order * DRAW_ENTRY_BYTES;
            (gsu_word(machine, base + DRAW_SHAPE) == SOURCE_PLAYER_LASER_SHAPE).then(|| LaserDraw {
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
                animation: machine.peek_gsu_ram(base + DRAW_ANIMATION),
            })
        })
        .collect()
}

fn native_laser_draws(shell: &Shell) -> Vec<LaserDraw> {
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
        .filter_map(|(list_order, draw)| {
            (draw.shape_id == NATIVE_PLAYER_LASER_SHAPE).then(|| {
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
                let position = sf_core::snes_trig::matrix_rotate_q15(
                    matrix, relative.0, relative.1, relative.2,
                );
                LaserDraw {
                    list_order,
                    position: Position(position.0, position.1, position.2),
                    rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                    animation: draw.anim_frame,
                }
            })
        })
        .collect()
}

fn main() {
    let rom = load_retail_rom().expect("Star Fox retail ROM is required");
    let mut retail = RetailMachine::new(rom);
    let mut native = support::configured_shell();
    let mut retail_level_boundary_aligned = false;
    let mut previous_retail_level_frame = None;
    let mut certified_weapon_updates = 0;
    let mut first_weapon_tick = None;
    let mut native_laser_sound_count = 0;
    let mut retail_laser_sound_count = 0;
    let mut first_draw_divergence = None;
    let mut first_scene_draw_divergence = None;
    let trace_timing = std::env::var_os("SF1_WEAPON_TRACE_TIMING").is_some();
    let probe_grid = std::env::var_os("SF1_WEAPON_GRID_PROBE").is_some();
    let probe_projection = std::env::var_os("SF1_WEAPON_PROJECTION_PROBE").is_some();
    let probe_objects = std::env::var_os("SF1_WEAPON_OBJECT_PROBE").is_some();
    let probe_polygons = std::env::var_os("SF1_WEAPON_POLYGON_PROBE").is_some();
    let probe_scanlines = std::env::var_os("SF1_WEAPON_SCANLINE_PROBE").is_some();
    let probe_bitmap = std::env::var_os("SF1_WEAPON_BITMAP_PROBE").is_some();
    let probe_sprite_plots = std::env::var_os("SF1_WEAPON_SPRITE_PLOT_PROBE").is_some();
    let probe_pixel_writes = std::env::var_os("SF1_WEAPON_PIXEL_WRITE_PROBE").is_some();
    let probe_presentation = std::env::var_os("SF1_WEAPON_PRESENTATION_PROBE").is_some();
    let probe_boundaries = std::env::var_os("SF1_WEAPON_BOUNDARY_PROBE").is_some();
    let probe_compositor = std::env::var_os("SF1_WEAPON_COMPOSITOR_PROBE").is_some();
    let mut compositor_capture_enabled = false;
    let bank_probe = std::env::var_os("SF1_CORNERIA_BANK_PROBE").is_some();
    let mut bank_certified_updates = 0u32;
    let mut previous_bank_offsets = None;
    let mut previous_retail_bank_offsets = None;
    let mut previous_retail_bank_roll = None;
    let mut previous_native_bank_roll = None;
    let mut first_bank_divergence = None;
    let probe_game_frame = std::env::var("SF1_WEAPON_PROBE_GAME_FRAME")
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("probe game frame must be decimal")
        })
        .unwrap_or(support::WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME);
    let source_video_first_game_frame = std::env::var(SOURCE_VIDEO_FIRST_FRAME_ENV)
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("source video first game frame must be decimal")
        })
        .unwrap_or(support::WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME);
    let source_video_last_game_frame = std::env::var(SOURCE_VIDEO_LAST_FRAME_ENV)
        .ok()
        .map(|value| {
            value
                .parse::<u16>()
                .expect("source video last game frame must be decimal")
        })
        .unwrap_or(support::WEAPON_VIDEO_CAPTURE_LAST_GAME_FRAME);
    assert!(
        source_video_first_game_frame <= source_video_last_game_frame,
        "source video game-frame range must be ordered"
    );
    let source_video_presentation_last_game_frame = source_video_last_game_frame
        .checked_add(1)
        .expect("source video presentation game frame must fit");
    let source_video_preroll_first_game_frame = source_video_first_game_frame
        .saturating_sub(2)
        .min(SOURCE_PRESENTATION_RETENTION_PREROLL_GAME_FRAME);
    let probed_local_x = std::env::var("SF1_WEAPON_PROBE_LOCAL_X")
        .ok()
        .map(|value| value.parse::<u16>().expect("probe local X must be decimal"))
        .unwrap_or(0);
    let probed_local_y = std::env::var("SF1_WEAPON_PROBE_LOCAL_Y")
        .ok()
        .map(|value| value.parse::<u16>().expect("probe local Y must be decimal"))
        .unwrap_or(68);
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(
        SOURCE_FRAME_WIDTH as i32,
        SOURCE_FRAME_HEIGHT as i32,
        &config_from_repo_root(repo_root),
    )
    .expect("headless weapon renderer");
    let mut pending_native_video: Option<(
        u16,
        sf_game::shell::FrameSnapshot,
        Vec<sf_render::draw_list::DrawListEntry>,
        Vec<support::NativeProjection>,
        Vec<u8>,
    )> = None;
    let mut completed_presentations = CompletedPresentationQueue::new();
    let mut completed_raster_capture_enabled = false;
    let mut completed_rasters = VecDeque::new();
    let mut pending_source_video = VecDeque::<PendingSourceVideo>::new();
    let mut first_bitmap_divergence = None;
    let mut sampled_bitmap_updates = 0u32;
    let mut first_internal_video_divergence: Option<SourceVideoDivergence> = None;
    let mut sampled_internal_video_updates = 0;
    let mut retail_frame_rate_range: Option<(u8, u8)> = None;
    let video_dump_directory: Option<std::path::PathBuf> =
        std::env::var_os("SF1_WEAPON_VIDEO_DUMP_DIR").map(Into::into);
    let dump_all_video = std::env::var_os("SF1_WEAPON_VIDEO_DUMP_ALL").is_some();
    let probe_raster_association =
        std::env::var_os("SF1_WEAPON_RASTER_ASSOCIATION_PROBE").is_some();
    if let Some(directory) = video_dump_directory.as_ref() {
        std::fs::create_dir_all(directory).expect("create weapon video dump directory");
    }
    if std::env::var_os("SF1_WEAPON_NATIVE_PRESENTATION_PROBE").is_some() {
        let mut scene = None;
        for tick in 0..=support::weapon_trace_end_tick() {
            native.tick(trace_input(tick, false));
            if native.state() != GameState::Playing
                || native.frame().gameplay_entry_phase != GameplayEntryPhase::ActiveLevel
            {
                continue;
            }
            let frame = native.frame();
            if frame.gameframe == probe_game_frame {
                println!(
                    "native_presentation_scene frame={} view_pitch={} view_yaw={} player_turn={} player_roll={} player_x={} background_base={} horizontal_horizon={:?} fixed_view={:?}",
                    frame.gameframe,
                    native.game.vars.strategy.view_pitch,
                    native.game.vars.strategy.view_yaw,
                    native.game.vars.strategy.player_turn_rotation,
                    native.game.vars.strategy.player_rotation[2],
                    native.game.vars.player_posx,
                    native.game.vars.shared.background_scroll_x,
                    &frame.bg2_horizontal_offsets.map(|offsets| [offsets[111], offsets[112]]),
                    native.game.vars.strategy.fixed_view_position,
                );
                scene = Some((
                    frame,
                    native.draw_list().iter().map(support::render_entry).collect::<Vec<_>>(),
                    support::native_source_projections(&native),
                    tick,
                ));
            } else if frame.gameframe == probe_game_frame.wrapping_add(1) {
                let (scene, draw_list, projections, scene_tick) =
                    scene.take().expect("native probe scene");
                let target = [usize::from(probed_local_x), usize::from(probed_local_y)];
                let target_offset = (target[1] * SOURCE_FRAME_WIDTH + target[0]) * 3;
                let retail_probe = std::env::var_os("SF1_WEAPON_RETAIL_FRAME_DIR").map(|directory| {
                    read_source_rgb_ppm(
                        std::path::PathBuf::from(directory)
                            .join(format!("retail-{}.ppm", scene.gameframe)),
                        0,
                    )
                    .expect("read retail probe frame")
                });
                let samples = (-8..=8)
                    .map(|adjustment| {
                        let mut adjusted = frame.clone();
                        adjusted.bg2_xscroll += adjustment;
                        if std::env::var_os("SF1_WEAPON_NATIVE_SCENE_BG_PITCH").is_some() {
                            adjusted.camera.rotation[0] = scene.camera.rotation[0];
                        }
                        let rgb = support::render_presentation_aligned_source_frame(
                            &scene,
                            &adjusted,
                            &draw_list,
                            &mut renderer,
                        );
                        if let Some(directory) =
                            std::env::var_os("SF1_WEAPON_NATIVE_ADJUSTMENT_DUMP_DIR")
                        {
                            write_source_rgb_ppm(
                                std::path::PathBuf::from(directory).join(format!(
                                    "native-{}-{adjustment:+}.ppm",
                                    scene.gameframe
                                )),
                                &rgb,
                            )
                            .expect("write native adjustment probe");
                        }
                        let divergence = retail_probe.as_ref().and_then(|retail| {
                            compare_source_rgb(0, 0, retail, &rgb)
                                .expect("compare native probe frame")
                        });
                        (
                            adjustment,
                            rgb[target_offset..target_offset + 3].to_vec(),
                            divergence,
                        )
                    })
                    .collect::<Vec<_>>();
                let best_sample = samples
                    .iter()
                    .min_by_key(|(_, _, divergence)| {
                        divergence
                            .as_ref()
                            .map_or(0, |divergence| divergence.differing_pixels)
                    })
                    .expect("presentation adjustment sample");
                println!(
                    "native_presentation_best scene_frame={} adjustment={} differing_pixels={} scene_x={} scene_yaw={} presentation_x={} presentation_yaw={}",
                    scene.gameframe,
                    best_sample.0,
                    best_sample
                        .2
                        .as_ref()
                        .map_or(0, |divergence| divergence.differing_pixels),
                    scene.camera.x >> 16,
                    scene.camera.rotation[1],
                    frame.camera.x >> 16,
                    frame.camera.rotation[1],
                );
                println!(
                    "native_presentation_probe scene_tick={scene_tick} presentation_tick={tick} scene_frame={} presentation_frame={} target={target:?} samples={samples:?} scene_bg2_xscroll={} presentation_bg2_xscroll={} scene_vertical={:?} presentation_vertical={:?} scene_horizontal_horizon={:?} presentation_horizontal_horizon={:?} scene_camera={:?} presentation_camera={:?}",
                    scene.gameframe,
                    frame.gameframe,
                    scene.bg2_xscroll,
                    frame.bg2_xscroll,
                    scene.bg2_vertical_offsets,
                    frame.bg2_vertical_offsets,
                    scene.bg2_horizontal_offsets.map(|offsets| [offsets[111], offsets[112]]),
                    frame.bg2_horizontal_offsets.map(|offsets| [offsets[111], offsets[112]]),
                    scene.camera,
                    frame.camera,
                );
                let source_x = usize::from(probed_local_x) + SOURCE_BITMAP_LEFT;
                let source_y = usize::from(probed_local_y) + SOURCE_BITMAP_TOP;
                let indices = renderer.source_bitmap_indices();
                let owners = renderer.source_bitmap_owners();
                let faces = renderer.source_bitmap_faces();
                let neighborhood = source_x
                    .saturating_sub(3)
                    .max(SOURCE_BITMAP_LEFT)
                    ..=(source_x + 3)
                        .min(SOURCE_BITMAP_LEFT + SOURCE_BITMAP_WIDTH - 1);
                println!(
                    "native_source_pixel_probe local={},{} neighborhood={:?} vertical={:?} projections={:?}",
                    probed_local_x,
                    probed_local_y,
                    neighborhood
                        .map(|x| {
                            let offset = source_y * SOURCE_FRAME_WIDTH + x;
                            (x - SOURCE_BITMAP_LEFT, indices[offset], owners[offset], faces[offset])
                        })
                        .collect::<Vec<_>>(),
                    (source_y.saturating_sub(8)..=(source_y + 8).min(SOURCE_FRAME_HEIGHT - 1))
                        .map(|y| {
                            let offset = y * SOURCE_FRAME_WIDTH + source_x;
                            (y - SOURCE_BITMAP_TOP, indices[offset], owners[offset], faces[offset])
                        })
                        .collect::<Vec<_>>(),
                    projections,
                );
                return;
            }
        }
        panic!("native presentation probe did not reach game frame {probe_game_frame}");
    }
    if std::env::var_os("SF1_WEAPON_NATIVE_PROBE").is_some() {
        for tick in 0..=support::weapon_trace_end_tick() {
            native.tick(trace_input(tick, false));
            if native.state() == GameState::Playing
                && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
                && native.game.vars.gameframe == probe_game_frame
            {
                let draw_list: Vec<_> = native.draw_list().iter().map(support::render_entry).collect();
                let _ = support::render_playing_snapshot(
                    &native.frame(),
                    &[],
                    &draw_list,
                    &mut renderer,
                );
                let pixel = (usize::from(probed_local_y) + SOURCE_BITMAP_TOP)
                    * SOURCE_FRAME_WIDTH
                    + usize::from(probed_local_x)
                    + SOURCE_BITMAP_LEFT;
                let probed_owner = native
                    .draw_list()
                    .iter()
                    .find(|draw| draw.shape_id == sf_render::shape_data::SHAPE_EXT_BOOSTSHAPE)
                    .map_or(0, |draw| draw.obj_id);
                let probed_owner_pixels = renderer
                    .source_bitmap_owners()
                    .iter()
                    .filter(|owner| **owner == probed_owner)
                    .count();
                let probed_owner_bounds = renderer
                    .source_bitmap_owners()
                    .iter()
                    .enumerate()
                    .filter(|(_, owner)| **owner == probed_owner)
                    .fold(None, |bounds, (offset, _)| {
                        let point = [offset % SOURCE_FRAME_WIDTH, offset / SOURCE_FRAME_WIDTH];
                        Some(bounds.map_or(
                            [point[0], point[1], point[0], point[1]],
                            |[left, top, right, bottom]: [usize; 4]| {
                                [
                                    left.min(point[0]),
                                    top.min(point[1]),
                                    right.max(point[0]),
                                    bottom.max(point[1]),
                                ]
                            },
                        ))
                    });
                println!(
                    "native_probe tick={tick} game_frame={probe_game_frame} index={} owner={} face={} bg2_xscroll={} camera={:?} boost_owner={probed_owner} boost_pixels={probed_owner_pixels} boost_bounds={probed_owner_bounds:?} source_draws={:#?} projections={:#?}",
                    renderer.source_bitmap_indices()[pixel],
                    renderer.source_bitmap_owners()[pixel],
                    renderer.source_bitmap_faces()[pixel],
                    native.frame().bg2_xscroll,
                    native.frame().camera,
                    support::native_source_draws(&native),
                    support::native_source_projections(&native),
                );
                return;
            }
        }
        panic!("native probe did not reach game frame {probe_game_frame}");
    }

    for tick in 0..=support::weapon_trace_end_tick() {
        let input = trace_input(tick, bank_probe);
        let next_input = trace_input(tick.saturating_add(1), bank_probe);
        let native_level_active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        if probe_compositor
            && !compositor_capture_enabled
            && native_level_active
            && native.game.vars.gameframe >= probe_game_frame.saturating_sub(4)
        {
            retail.capture_completed_bg1_indices();
            compositor_capture_enabled = true;
        }
        if !completed_raster_capture_enabled
            && native_level_active
            && native.game.vars.gameframe >= source_video_preroll_first_game_frame
        {
            retail.capture_completed_rasters();
            completed_raster_capture_enabled = true;
        }
        let align_completed_level_frame =
            native_level_active && tick >= COMPLETED_FRAME_ALIGNMENT_TICK;
        let mut retail_draws_for_update = None;
        let mut retail_scene_draws_for_update = None;
        let mut retail_view_for_update = None;
        let mut retail_sound_events_for_update = None;
        let mut retail_video_for_update = None;
        let mut retail_horizontal_for_update = None;
        let mut retail_projection_captures_for_update = None;
        let mut retail_pixel_write_captures_for_update = None;
        let mut retail_bank_for_update = None;
        if align_completed_level_frame {
            if !retail_level_boundary_aligned {
                assert!(
                    retail
                        .tick_until_cpu_execution(
                            input,
                            RETAIL_DOSTRATS,
                            MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE,
                        )
                        .expect("initial gameplay boundary"),
                    "retail did not reach the initial gameplay boundary"
                );
                retail_level_boundary_aligned = true;
            }
            let retail_frame_rate = retail.peek8(WORK_RAM | RETAIL_FRAMERATE);
            retail_frame_rate_range = Some(retail_frame_rate_range.map_or(
                (retail_frame_rate, retail_frame_rate),
                |(minimum, maximum)| {
                    (
                        minimum.min(retail_frame_rate),
                        maximum.max(retail_frame_rate),
                    )
                },
            ));
            let retail_sound_cursor = retail.peek8(WORK_RAM | RETAIL_SOUND_EFFECT_WRITE_CURSOR);
            let max_video_frames = if tick == CORNERIA_AUDIO_UPLOAD_TICK {
                MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD
            } else {
                MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE
            };
            assert!(
                retail
                    .tick_until_cpu_execution(input, RETAIL_BUILD_DRAWLIST_L, max_video_frames)
                    .expect("completed gameplay draw boundary"),
                "retail did not complete gameplay draw update {tick}"
            );
            retail_draws_for_update = Some(retail_laser_draws(&retail));
            retail_scene_draws_for_update = Some(support::retail_source_draws(&retail));
            if bank_probe {
                retail_bank_for_update = retail_bg2_vertical_offsets(&retail).map(|offsets| {
                    (
                        retail.peek16(WORK_RAM | RETAIL_PLROTZ) as i16,
                        offsets,
                    )
                });
            }
            retail_view_for_update = Some(Position(
                retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_X) as i16,
                retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Y) as i16,
                retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Z) as i16,
            ));
            let next_retail_sound_cursor =
                retail.peek8(WORK_RAM | RETAIL_SOUND_EFFECT_WRITE_CURSOR);
            retail_sound_events_for_update = Some(retail_sound_events(
                &retail,
                retail_sound_cursor,
                next_retail_sound_cursor,
            ));
            let capture_projection =
                (probe_projection
                    || probe_objects
                    || probe_polygons
                    || probe_scanlines
                    || probe_sprite_plots)
                && native.game.vars.gameframe.wrapping_add(1) == probe_game_frame;
            let capture_pixel_writes = probe_pixel_writes
                && native.game.vars.gameframe.wrapping_add(1) == probe_game_frame;
            if capture_projection {
                let (program_bank, instruction) = if probe_objects {
                    (GSU_OBJECT_START_BANK, GSU_OBJECT_START_INSTRUCTION)
                } else if probe_sprite_plots {
                    (
                        GSU_REDUCED_SPRITE_PLOT_BANK,
                        GSU_REDUCED_SPRITE_PLOT_INSTRUCTION,
                    )
                } else if probe_scanlines {
                    (GSU_SCANLINE_START_BANK, GSU_SCANLINE_START_INSTRUCTION)
                } else if probe_polygons {
                    (GSU_POLYGON_START_BANK, GSU_POLYGON_START_INSTRUCTION)
                } else {
                    (
                        GSU_PROJECTION_COMPLETE_BANK,
                        GSU_PROJECTION_COMPLETE_INSTRUCTION,
                    )
                };
                retail.watch_gsu_execution_capture(
                    program_bank,
                    instruction,
                    GSU_CAPTURE_START,
                    GSU_CAPTURE_LENGTH,
                );
            }
            if capture_pixel_writes {
                retail.watch_gsu_pixel_writes(
                    u8::try_from(probed_local_x).expect("probe local X must fit source bitmap"),
                    u8::try_from(probed_local_y).expect("probe local Y must fit source bitmap"),
                );
            }
            assert!(
                retail
                    .tick_until_cpu_execution(next_input, RETAIL_DOSTRATS, max_video_frames,)
                    .expect("next gameplay boundary"),
                "retail did not reach the next gameplay update {tick}"
            );
            if capture_projection {
                retail_projection_captures_for_update =
                    Some(retail.take_gsu_execution_captures());
            }
            if capture_pixel_writes {
                retail_pixel_write_captures_for_update =
                    Some(retail.take_gsu_pixel_write_captures());
            }
            let retail_ppu = retail.ppu_frame();
            retail_horizontal_for_update = Some(std::array::from_fn::<_, SOURCE_FRAME_HEIGHT, _>(
                |row| retail_ppu.scanline_bg_hofs[row][1] as i16,
            ));
            retail_video_for_update = Some((retail.video_frame(), source_rgb(retail_ppu)));
        } else {
            retail
                .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
                .expect("retail front-end update");
        }
        if completed_raster_capture_enabled {
            completed_rasters.extend(retail.take_completed_rasters());
        }
        let retail_level_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let retail_completed_level_update = align_completed_level_frame
            || previous_retail_level_frame
                .map(|previous| previous != retail_level_frame)
                .unwrap_or(true);
        if !native_level_active || retail_completed_level_update {
            native.tick(input);
        }
        if native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
        {
            previous_retail_level_frame = Some(retail_level_frame);
        }
        if std::env::var_os("SF1_WEAPON_RETAIL_HORIZONTAL_TABLE_PROBE").is_some()
            && native.game.vars.gameframe == probe_game_frame.wrapping_add(1)
        {
            let frame = retail.ppu_frame();
            println!(
                "retail_horizontal_table_probe source_game_frame={probe_game_frame} native_game_frame={} retail_game_frame={retail_level_frame} video_frame={} rows={:?}",
                native.game.vars.gameframe,
                retail.video_frame(),
                frame
                    .scanline_bg_hofs
                    .iter()
                    .enumerate()
                    .map(|(row, offsets)| (row, offsets[1]))
                    .collect::<Vec<_>>(),
            );
            return;
        }

        if bank_probe && align_completed_level_frame {
            let Some((retail_roll, retail_offsets)) = retail_bank_for_update else {
                continue;
            };
            let native_roll = native.game.vars.strategy.player_rotation[2];
            assert_eq!(
                native_roll, retail_roll,
                "Corneria player bank at game frame {}",
                native.game.vars.gameframe
            );
            {
                let native_offsets = native
                    .frame()
                    .bg2_vertical_offsets
                    .expect("native Corneria vertical offset columns enabled");
                if first_bank_divergence.is_none() && native_offsets != retail_offsets {
                    first_bank_divergence = Some((
                        native.game.vars.gameframe,
                        retail_roll,
                        native_roll,
                        previous_retail_bank_roll,
                        previous_native_bank_roll,
                        retail_offsets,
                        native_offsets,
                    ));
                }
                if previous_retail_bank_offsets != Some(retail_offsets) {
                    println!(
                        "corneria_bank retail_game_frame={} player_roll={} offsets={retail_offsets:?}",
                        native.game.vars.gameframe,
                        retail_roll,
                    );
                    previous_retail_bank_offsets = Some(retail_offsets);
                }
                if previous_bank_offsets != Some(native_offsets) {
                    println!(
                        "corneria_bank game_frame={} player_roll={} offsets={native_offsets:?}",
                        native.game.vars.gameframe,
                        native_roll,
                    );
                    previous_bank_offsets = Some(native_offsets);
                }
                bank_certified_updates += 1;
                if native.game.vars.gameframe >= BANK_PROBE_LAST_GAME_FRAME {
                    match first_bank_divergence {
                        Some(divergence) => println!(
                            "corneria_bank compared_updates={bank_certified_updates} first_divergence={divergence:?}"
                        ),
                        None => println!(
                            "corneria_bank certified_updates={bank_certified_updates} first_divergence=none runtime_source_coverage=calcbg2voffsets,bg2tab1,bg2tab2,bg2tab4,bg2tab6"
                        ),
                    }
                    assert_eq!(
                        first_bank_divergence, None,
                        "authoritative retail Corneria bank table diverged"
                    );
                    return;
                }
            }
            previous_retail_bank_roll = Some(retail_roll);
            previous_native_bank_roll = Some(native_roll);
        }

        // Banking is a standalone visual-conformance scenario. Its input is
        // intentionally different from the weapon trace, so none of the
        // weapon-specific state, draw, or camera assertions apply here.
        if bank_probe {
            continue;
        }

        if trace_timing && (tick >= FIRST_LEVEL_STATE_TICK || tick + 1 == FIRST_LEVEL_STATE_TICK) {
            println!(
                "timing tick={tick} input={input} retail_game_frame={retail_level_frame} retail_video_frame={} native_game_frame={} native_active={native_level_active}",
                retail.video_frame(),
                native.game.vars.gameframe,
            );
        }

        let current_native_draw_list: Vec<_> = native
            .draw_list()
            .iter()
            .map(support::render_entry)
            .collect();
        if native_level_active
            && (source_video_preroll_first_game_frame..=source_video_presentation_last_game_frame)
                .contains(&native.game.vars.gameframe)
        {
            let game_frame = native.game.vars.gameframe;
            let prepares_native_frame = (source_video_preroll_first_game_frame
                ..=source_video_last_game_frame)
                .contains(&game_frame);
            let captures_native_frame = (source_video_first_game_frame
                ..=source_video_last_game_frame)
                .contains(&game_frame);
            if probe_boundaries && captures_native_frame {
                let frame = retail_video_for_update
                    .as_ref()
                    .expect("retail presentation at gameplay boundary");
                let bg_indices = retail.ppu_snapshot_bg_indices(0);
                let pixel_offset =
                    (TARGET_DISPLAY_PIXEL[1] * SOURCE_FRAME_WIDTH + TARGET_DISPLAY_PIXEL[0]) * 3;
                println!(
                    "boundary_probe native_game_frame={game_frame} retail_game_frame={} video_frame={} live_bg_pixel={} completed_color={:?} completed_hash={:016X} ppu_hofs={} source_scroll={} view_yaw={} player_turn={} background_base={}",
                    retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
                    frame.0,
                    bg_indices[TARGET_DISPLAY_PIXEL[1] * SOURCE_FRAME_WIDTH + TARGET_DISPLAY_PIXEL[0]],
                    &frame.1[pixel_offset..pixel_offset + 3],
                    sf_difftest::hash_rgb(&frame.1),
                    retail.ppu_frame().bg_hofs[1],
                    gsu_word(&retail, GSU_HORIZONTAL_SCROLL_OFFSET) as i16,
                    retail.peek16(WORK_RAM | 0x0548) as i16,
                    retail.peek16(WORK_RAM | 0x0510) as i16,
                    retail.peek16(WORK_RAM | 0x1F30) as i16,
                );
            }
            if probe_grid && game_frame == source_video_first_game_frame {
                let camera = native.frame().camera;
                println!(
                    "grid_probe native_camera={camera:?} native_pixels={:?}",
                    native.frame().point_pixels,
                );
                println!(
                    "grid_probe retail_world_matrix={:?} retail_grid_preparation={:?}",
                    (0..9)
                        .map(|index| gsu_word(&retail, GSU_WORLD_MATRIX + index * 2) as i16)
                        .collect::<Vec<_>>(),
                    (0..9)
                        .map(|index| gsu_word(&retail, GSU_GRID_PREPARATION + index * 2) as i16)
                        .collect::<Vec<_>>(),
                );
                let ppu = retail.ppu_frame();
                println!(
                    "grid_probe retail_framebuffer_palette={:?}",
                    ppu.cgram[7 * 32..8 * 32]
                        .chunks_exact(2)
                        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
                        .collect::<Vec<_>>(),
                );
                println!(
                    "grid_probe retail_palette_fourteen_pixels={:?}",
                    retail_bitmap_indices(&retail)
                        .into_iter()
                        .enumerate()
                        .filter_map(|(offset, index)| (index == 14).then_some([
                            offset % SOURCE_BITMAP_WIDTH,
                            offset / SOURCE_BITMAP_WIDTH,
                        ]))
                        .collect::<Vec<_>>(),
                );
                let layer_indices: Vec<_> = (0..4)
                    .map(|background| retail.ppu_snapshot_bg_indices(background))
                    .collect();
                for (label, x, y) in [
                    ("meter_outline", 24usize, 192usize),
                    ("meter_fill", 26, 194),
                    ("boost_fill", 194, 194),
                    ("building", 16, 84),
                    ("player", 128, 113),
                ] {
                    println!(
                        "grid_probe pixel={label} position={x},{y} layer_indices={:?}",
                        layer_indices
                            .iter()
                            .map(|indices| indices[y * SOURCE_FRAME_WIDTH + x])
                            .collect::<Vec<_>>(),
                    );
                }
            }
            if probe_projection && game_frame == probe_game_frame {
                for (index, capture) in retail_projection_captures_for_update
                    .as_ref()
                    .expect("retail projection captures")
                    .iter()
                    .enumerate()
                {
                    println!(
                        "projection_probe capture={index} {}",
                        describe_projection_capture(capture)
                    );
                }
                return;
            }
            if probe_objects && game_frame == probe_game_frame {
                let captures = retail_projection_captures_for_update
                    .as_ref()
                    .expect("retail object captures");
                println!(
                    "object_probe retail_draws={:#?} native_draws={:#?}",
                    retail_scene_draws_for_update
                        .as_ref()
                        .expect("retail object-probe draw chain"),
                    support::native_source_draws(&native),
                );
                println!(
                    "object_probe capture_count={} objects={:?}",
                    captures.len(),
                    captures
                        .iter()
                        .map(|capture| {
                            (
                                projection_capture_position(capture),
                                captured_word(&capture.memory, GSU_SHAPE_POINTER),
                                captured_word(&capture.memory, GSU_FACE_POINTER),
                                capture.values,
                            )
                        })
                        .collect::<Vec<_>>(),
                );
                return;
            }
            if probe_polygons && game_frame == probe_game_frame {
                let target_position = if std::env::var_os("SF1_WEAPON_LASER_POLYGON_PROBE")
                    .is_some()
                {
                    [-3, 7, 479]
                } else if std::env::var_os("SF1_WEAPON_SHADOW_POLYGON_PROBE").is_some() {
                    [0, 54, 166]
                } else {
                    TARGET_BUILDING_VIEW_POSITION
                };
                let captures = retail_projection_captures_for_update
                    .as_ref()
                    .expect("retail polygon captures");
                println!(
                    "polygon_probe capture_count={} positions={:?}",
                    captures.len(),
                    captures
                        .iter()
                        .map(projection_capture_position)
                        .collect::<Vec<_>>(),
                );
                for (index, capture) in captures
                    .iter()
                    .filter(|capture| projection_capture_position(capture) == target_position)
                    .enumerate()
                {
                    println!(
                        "polygon_probe capture={index} {}",
                        describe_polygon_capture(capture)
                    );
                }
                let ppu = retail.ppu_frame();
                println!(
                    "polygon_probe display_scroll horizontal={:?} vertical={:?} registers={:02X?}",
                    ppu.bg_hofs,
                    ppu.bg_vofs,
                    [
                        ppu.registers[0x05],
                        ppu.registers[0x07],
                        ppu.registers[0x08],
                        ppu.registers[0x2C],
                        ppu.registers[0x30],
                    ],
                );
                for background in 0..4 {
                    let rgb = retail
                        .ppu_snapshot_bg_rgba(background)
                        .chunks_exact(4)
                        .flat_map(|pixel| pixel[..3].iter().copied())
                        .collect::<Vec<_>>();
                    write_source_rgb_ppm(
                        format!("/tmp/sf1-weapon-retail-bg{}.ppm", background + 1),
                        &rgb,
                    )
                    .expect("write retail weapon background layer");
                }
                return;
            }
            if probe_scanlines && game_frame == probe_game_frame {
                let captures = retail_projection_captures_for_update
                    .as_ref()
                    .expect("retail scanline captures");
                for (order, capture) in captures.iter().enumerate().filter(|(_, capture)| {
                    capture.values[2] == probed_local_y
                        && capture.values[7] >> 8 <= probed_local_x
                        && capture.values[9] >> 8 >= probed_local_x
                }) {
                    println!(
                        "scanline_probe writer_order={order} pixel={probed_local_x},{probed_local_y} view={:?} shape_pointer={:04X} edge_x={},{} color={}",
                        projection_capture_position(capture),
                        captured_word(&capture.memory, GSU_SHAPE_POINTER),
                        capture.values[7] >> 8,
                        capture.values[9] >> 8,
                        capture.color,
                    );
                }
                let ppu_indices = retail.ppu_snapshot_bg_indices(0);
                println!(
                    "scanline_probe bitmap_pixel={} displayed_bg_pixel={} gsu_state={:?}",
                    retail_bitmap_pixel(&retail, probed_local_x as u8, probed_local_y as u8),
                    ppu_indices[TARGET_DISPLAY_PIXEL[1] * SOURCE_FRAME_WIDTH + TARGET_DISPLAY_PIXEL[0]],
                    retail.gsu_screen_state(),
                );
                return;
            }
            if probe_bitmap && game_frame == probe_game_frame {
                let frame = native.frame();
                let _ = support::render_playing_snapshot(
                    &frame,
                    &[],
                    &current_native_draw_list,
                    &mut renderer,
                );
                let source_indices = renderer.source_bitmap_indices();
                let source_owners = renderer.source_bitmap_owners();
                let source_faces = renderer.source_bitmap_faces();
                let local_left = usize::from(probed_local_x).saturating_sub(4);
                let local_right = (usize::from(probed_local_x) + 4).min(SOURCE_BITMAP_WIDTH - 1);
                let local_top = usize::from(probed_local_y).saturating_sub(4);
                let local_bottom =
                    (usize::from(probed_local_y) + 4).min(SOURCE_BITMAP_HEIGHT - 1);
                let mut samples = Vec::new();
                for y in local_top..=local_bottom {
                    for x in local_left..=local_right {
                        let native_offset = (y + SOURCE_BITMAP_TOP) * SOURCE_FRAME_WIDTH
                            + x
                            + SOURCE_BITMAP_LEFT;
                        let sample = (
                            x,
                            y,
                            retail_bitmap_pixel(&retail, x as u8, y as u8),
                            source_indices[native_offset],
                            source_owners[native_offset],
                            source_faces[native_offset],
                        );
                        if sample.2 != sample.3 || sample.2 != 0 || sample.3 != 0 {
                            samples.push(sample);
                        }
                    }
                }
                println!(
                    "bitmap_probe game_frame={game_frame} local={probed_local_x},{probed_local_y} samples={samples:?}"
                );
                return;
            }
            if probe_pixel_writes && game_frame == probe_game_frame {
                let captures = retail_pixel_write_captures_for_update
                    .as_ref()
                    .expect("retail pixel write captures");
                for (order, capture) in captures.iter().enumerate() {
                    println!(
                        "pixel_write_probe order={order} pixel={probed_local_x},{probed_local_y} instruction={:06X} color={} values={:04X?} view={:?} shift={} vanish={:?} texture_mode={} color_frame={} color_table={:04X} texture_color_mode={:04X} sprite_bank={:04X} sprite_data={:04X} authored_extent={} clip={:?} center={:?} source_size={} projected_size={} mask={:04X} material_index={}",
                        capture.instruction,
                        capture.color,
                        capture.values,
                        projection_capture_position(capture),
                        captured_word(&capture.memory, GSU_SHAPE_SHIFT),
                        [
                            captured_word(&capture.memory, GSU_VANISHING_POINT_X),
                            captured_word(&capture.memory, GSU_VANISHING_POINT_Y),
                        ],
                        captured_word(&capture.memory, GSU_TEXTURE_MODE),
                        captured_word(&capture.memory, GSU_COLOR_FRAME),
                        captured_word(&capture.memory, GSU_COLOR_TABLE_POINTER),
                        captured_word(&capture.memory, GSU_TEXTURE_COLOR_MODE),
                        captured_word(&capture.memory, GSU_SPRITE_BANK),
                        captured_word(&capture.memory, GSU_SPRITE_DATA),
                        captured_word(&capture.memory, GSU_SPRITE_AUTHORED_EXTENT),
                        [
                            captured_word(&capture.memory, GSU_SPRITE_LEFT_CLIP) as i16,
                            captured_word(&capture.memory, GSU_SPRITE_RIGHT_CLIP) as i16,
                        ],
                        [
                            captured_word(&capture.memory, GSU_SPRITE_X) as i16,
                            captured_word(&capture.memory, GSU_SPRITE_Y) as i16,
                        ],
                        captured_word(&capture.memory, GSU_SPRITE_SOURCE_SIZE),
                        captured_word(&capture.memory, GSU_SPRITE_PROJECTED_SIZE),
                        captured_word(&capture.memory, GSU_SPRITE_MASK),
                        captured_word(&capture.memory, GSU_SPRITE_MATERIAL_INDEX),
                    );
                }
                println!(
                    "pixel_write_probe writes={} bitmap_pixel={}",
                    captures.len(),
                    retail_bitmap_pixel(&retail, probed_local_x as u8, probed_local_y as u8),
                );
                return;
            }
            if probe_sprite_plots && game_frame == probe_game_frame {
                let captures = retail_projection_captures_for_update
                    .as_ref()
                    .expect("retail sprite plot captures");
                for (order, capture) in captures.iter().enumerate().filter(|(_, capture)| {
                    capture.values[1] == probed_local_x && capture.values[2] == probed_local_y
                }) {
                    println!(
                        "sprite_plot_probe order={order} pixel={probed_local_x},{probed_local_y} color={} texture_address={:04X} center={:?} source_size={} projected_size={} texture_color_mode={:04X}",
                        capture.color,
                        capture.values[14],
                        [
                            captured_word(&capture.memory, GSU_SPRITE_X) as i16,
                            captured_word(&capture.memory, GSU_SPRITE_Y) as i16,
                        ],
                        captured_word(&capture.memory, GSU_SPRITE_SOURCE_SIZE),
                        captured_word(&capture.memory, GSU_SPRITE_PROJECTED_SIZE),
                        captured_word(&capture.memory, GSU_TEXTURE_COLOR_MODE),
                    );
                }
                println!("sprite_plot_probe captures={}", captures.len());
                return;
            }
            if probe_presentation && game_frame == source_video_first_game_frame {
                for step in 0..=PRESENTATION_PROBE_VIDEO_FRAMES {
                    let frame = retail.ppu_frame();
                    let target = [usize::from(probed_local_x), usize::from(probed_local_y)];
                    let target_offset = target[1] * SOURCE_FRAME_WIDTH + target[0];
                    let layer_indices: [u8; 4] = std::array::from_fn(|background| {
                        retail.ppu_snapshot_bg_indices(background)[target_offset]
                    });
                    let layer_colors: [[u8; 3]; 4] = std::array::from_fn(|background| {
                        let rgba = retail.ppu_snapshot_bg_rgba(background);
                        let pixel = &rgba[target_offset * 4..target_offset * 4 + 3];
                        [pixel[0], pixel[1], pixel[2]]
                    });
                    let cgram_words: [u16; 4] = layer_indices.map(|index| {
                        let offset = usize::from(index) * 2;
                        u16::from_le_bytes([frame.cgram[offset], frame.cgram[offset + 1]])
                    });
                    let pixel_offset = target_offset * 4;
                    println!(
                        "presentation_probe step={step} video_frame={} game_frame={} target={target:?} layer_indices={layer_indices:?} layer_colors={layer_colors:?} cgram_words={cgram_words:04X?} color={:?} scroll_h={:?} scroll_v={:?} vertical_offsets={:?} view_position={:?}",
                        retail.video_frame(),
                        retail.peek16(WORK_RAM | RETAIL_GAMEFRAME),
                        &frame.rgba[pixel_offset..pixel_offset + 3],
                        frame.bg_hofs,
                        frame.bg_vofs,
                        retail_bg2_vertical_offsets(&retail),
                        [
                            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_X) as i16,
                            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Y) as i16,
                            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Z) as i16,
                        ],
                    );
                    if step < PRESENTATION_PROBE_VIDEO_FRAMES {
                        retail
                            .tick_video_frames(next_input, 1)
                            .expect("advance retail presentation probe");
                    }
                }
                return;
            }
            if captures_native_frame {
                let native_scene_draws = support::native_source_draws(&native);
                let retail_scene_draws = retail_scene_draws_for_update
                    .as_ref()
                    .expect("aligned retail scene draw list");
                if first_scene_draw_divergence.is_none()
                    && native_scene_draws != *retail_scene_draws
                {
                    let differences = (0..native_scene_draws.len().max(retail_scene_draws.len()))
                        .filter_map(|index| {
                            let native_draw = native_scene_draws.get(index).copied();
                            let retail_draw = retail_scene_draws.get(index).copied();
                            (native_draw != retail_draw).then_some((index, native_draw, retail_draw))
                        })
                        .collect::<Vec<_>>();
                    first_scene_draw_divergence = Some((
                        tick,
                        game_frame,
                        native_scene_draws.len(),
                        retail_scene_draws.len(),
                        differences,
                    ));
                }
            }
            // The SNES displays the completed Super FX bitmap at the next
            // gameplay boundary after it was drawn. Pair that observable
            // presentation with the pending native frame; never substitute
            // or resynchronize either machine's state.
            if let Some((
                pending_game_frame,
                pending_frame,
                pending_draw_list,
                pending_projections,
                retail_bitmap,
            )) = pending_native_video.take()
            {
                assert_eq!(
                    game_frame,
                    pending_game_frame + 1,
                    "retail bitmap presentation phase"
                );
                let presentation_frame = native.frame();
                if std::env::var_os("SF1_WEAPON_RETAIL_HORIZONTAL_TABLE_CENSUS").is_some() {
                    let retail_horizontal = retail_horizontal_for_update
                        .as_ref()
                        .expect("presentation-aligned retail horizontal table");
                    println!(
                        "horizontal_table_census game_frame={pending_game_frame} retail_horizon={:?} scene_horizon={:?} presentation_horizon={:?} retail_runs={:?}",
                        [retail_horizontal[111], retail_horizontal[112]],
                        pending_frame
                            .bg2_horizontal_offsets
                            .map(|offsets| [offsets[111], offsets[112]]),
                        presentation_frame
                            .bg2_horizontal_offsets
                            .map(|offsets| [offsets[111], offsets[112]]),
                        retail_horizontal
                            .iter()
                            .copied()
                            .fold(Vec::<(i16, usize)>::new(), |mut runs, value| {
                                if let Some((_, count)) =
                                    runs.last_mut().filter(|(last, _)| *last == value)
                                {
                                    *count += 1;
                                } else {
                                    runs.push((value, 1));
                                }
                                runs
                            }),
                    );
                }
                let native_rgb = support::render_presentation_aligned_source_frame(
                    &pending_frame,
                    &presentation_frame,
                    &pending_draw_list,
                    &mut renderer,
                );
                let releases_completed_scene = pending_frame.display_forced_blank
                    && !presentation_frame.display_forced_blank
                    && pending_frame.screen_wipe.active;
                let presented_native_rgb = completed_presentations
                    .advance(
                        native_rgb,
                        releases_completed_scene,
                        presentation_frame.windowmode != 0,
                    )
                    .expect("completed source presentation")
                    .value;
                if probe_compositor && pending_game_frame == probe_game_frame {
                    let ppu = retail.ppu_frame();
                    let bg1 = retail.ppu_snapshot_bg_indices(0);
                    let (_, completed_rgb) = retail_video_for_update
                        .as_ref()
                        .expect("presentation-aligned retail source frame");
                    let sample = |x: usize, y: usize| {
                        let pixel = (y * SOURCE_FRAME_WIDTH + x) * 3;
                        [
                            completed_rgb[pixel],
                            completed_rgb[pixel + 1],
                            completed_rgb[pixel + 2],
                        ]
                    };
                    println!(
                        "compositor_probe game_frame={pending_game_frame} bg_hofs={:?} bg_vofs={:?} registers={:02X?} raw_x0={:?} bg1_x16={:?} scanout_bg1_x16={:?} scanout_priority_x16={:?} completed_x16={:?}",
                        ppu.bg_hofs,
                        ppu.bg_vofs,
                        [
                            ppu.registers[0x05],
                            ppu.registers[0x07],
                            ppu.registers[0x0B],
                            ppu.registers[0x2C],
                            ppu.registers[0x2E],
                        ],
                        (84..=108)
                            .map(|y| (y, retail_bitmap[y * SOURCE_BITMAP_WIDTH]))
                            .collect::<Vec<_>>(),
                        (100..=124)
                            .map(|y| (y, bg1[y * SOURCE_FRAME_WIDTH + SOURCE_BITMAP_LEFT]))
                            .collect::<Vec<_>>(),
                        (100..=124)
                            .map(|y| {
                                (
                                    y,
                                    ppu.completed_bg1_indices
                                        [y * SOURCE_FRAME_WIDTH + SOURCE_BITMAP_LEFT],
                                )
                            })
                            .collect::<Vec<_>>(),
                        (100..=124)
                            .map(|y| {
                                (
                                    y,
                                    ppu.completed_bg_priority
                                        [y * SOURCE_FRAME_WIDTH + SOURCE_BITMAP_LEFT],
                                )
                            })
                            .collect::<Vec<_>>(),
                        (100..=124)
                            .map(|y| (y, sample(SOURCE_BITMAP_LEFT, y)))
                            .collect::<Vec<_>>(),
                    );
                    return;
                }
                let native_bitmap = renderer.source_bitmap_indices();
                let native_owners = renderer.source_bitmap_owners();
                let native_faces = renderer.source_bitmap_faces();
                let mut compared_pixels = 0usize;
                let mut differing_pixels = 0usize;
                let mut first_pixel = None;
                let point_pixels = pending_frame
                    .point_pixels
                    .iter()
                    .map(|pixel| (usize::from(pixel.x), usize::from(pixel.y)))
                    .collect::<std::collections::HashSet<_>>();
                let mut upper_scene_differing_pixels = 0usize;
                let mut first_upper_scene_pixel = None;
                for y in 0..SOURCE_BITMAP_HEIGHT {
                    for x in 0..SOURCE_BITMAP_WIDTH {
                        let native_offset = (y + SOURCE_BITMAP_TOP) * SOURCE_FRAME_WIDTH
                            + x
                            + SOURCE_BITMAP_LEFT;
                        let native_index = native_bitmap[native_offset];
                        compared_pixels += 1;
                        let retail_index = retail_bitmap[y * SOURCE_BITMAP_WIDTH + x];
                        if native_index != retail_index {
                            differing_pixels += 1;
                            first_pixel.get_or_insert((
                                x,
                                y,
                                retail_index,
                                native_index,
                                native_owners[native_offset],
                                native_faces[native_offset],
                            ));
                            if y < 150 && !point_pixels.contains(&(x, y)) {
                                upper_scene_differing_pixels += 1;
                                first_upper_scene_pixel.get_or_insert((
                                    x,
                                    y,
                                    retail_index,
                                    native_index,
                                    native_owners[native_offset],
                                    native_faces[native_offset],
                                ));
                            }
                        }
                    }
                }
                if probe_grid && pending_game_frame == source_video_first_game_frame {
                    println!(
                        "grid_probe upper_scene_differing_pixels={upper_scene_differing_pixels} first_upper_scene_pixel={first_upper_scene_pixel:?}"
                    );
                }
                if pending_game_frame >= source_video_first_game_frame
                    && first_bitmap_divergence.is_none()
                    && differing_pixels != 0
                {
                    let first_pixel = first_pixel.expect("bitmap divergence pixel");
                    let owner_draw = pending_draw_list
                        .iter()
                        .find(|draw| draw.obj_id == first_pixel.4)
                        .copied();
                    let player_projection = pending_projections
                        .iter()
                        .find(|projection| projection.shape == sf_render::shapes::SHAPE_ARWING)
                        .cloned();
                    first_bitmap_divergence = Some((
                        pending_game_frame,
                        compared_pixels,
                        differing_pixels,
                        first_pixel,
                        owner_draw,
                        player_projection,
                        (
                            [
                                pending_frame.camera.x,
                                pending_frame.camera.y,
                                pending_frame.camera.z,
                            ],
                            pending_frame.camera.rotation,
                        ),
                    ));
                }
                if pending_game_frame >= source_video_first_game_frame {
                    sampled_bitmap_updates += 1;
                    pending_source_video.push_back(PendingSourceVideo {
                        game_frame: pending_game_frame,
                        native_rgb: presented_native_rgb,
                        retail_bitmap: retail_bitmap.clone(),
                    });
                }
            }
            if prepares_native_frame {
                pending_native_video = Some((
                    game_frame,
                    native.frame(),
                    current_native_draw_list.clone(),
                    support::native_source_projections(&native),
                    retail_bitmap_indices(&retail),
                ));
            }
        }

        while let Some(candidate) = pending_source_video.front() {
            let Some(matched_position) = completed_rasters.iter().position(|raster| {
                completed_raster_contains_bitmap(raster, &candidate.retail_bitmap)
            }) else {
                if probe_raster_association {
                    println!(
                        "raster_association_probe game_frame={} candidates={:?}",
                        candidate.game_frame,
                        completed_rasters
                            .iter()
                            .map(|raster| (
                                raster.video_frame,
                                completed_raster_bitmap_differences(
                                    raster,
                                    &candidate.retail_bitmap,
                                )
                            ))
                            .collect::<Vec<_>>(),
                    );
                }
                completed_rasters.clear();
                break;
            };
            for _ in 0..matched_position {
                completed_rasters.pop_front();
            }
            let raster = completed_rasters
                .pop_front()
                .expect("matched completed retail raster");
            let candidate = pending_source_video
                .pop_front()
                .expect("matched pending source video");
            let retail_rgb = completed_raster_rgb(&raster);
            if dump_all_video {
                let dump_directory = video_dump_directory
                    .as_ref()
                    .expect("SF1_WEAPON_VIDEO_DUMP_ALL requires a dump directory");
                write_source_rgb_ppm(
                    dump_directory.join(format!("retail-{:03}.ppm", candidate.game_frame)),
                    &retail_rgb,
                )
                .expect("write retail weapon video census");
                write_source_rgb_ppm(
                    dump_directory.join(format!("native-{:03}.ppm", candidate.game_frame)),
                    &candidate.native_rgb,
                )
                .expect("write native weapon video census");
            }
            if first_internal_video_divergence.is_none() {
                let divergence = compare_source_rgb(
                    u64::from(candidate.game_frame - source_video_first_game_frame),
                    raster.video_frame,
                    &retail_rgb,
                    &candidate.native_rgb,
                )
                .expect("compare weapon source video");
                if divergence.is_some() {
                    if let Some(dump_directory) = video_dump_directory.as_ref() {
                        write_source_rgb_ppm(
                            dump_directory.join(format!("retail-{:03}.ppm", candidate.game_frame)),
                            &retail_rgb,
                        )
                        .expect("write retail weapon video diagnostic");
                        write_source_rgb_ppm(
                            dump_directory.join(format!("native-{:03}.ppm", candidate.game_frame)),
                            &candidate.native_rgb,
                        )
                        .expect("write native weapon video diagnostic");
                    }
                }
                first_internal_video_divergence = divergence;
            }
            sampled_internal_video_updates += 1;
        }
        if pending_source_video.is_empty() {
            completed_rasters.clear();
        }

        let sounds = native.drain_sound();
        let native_laser_events = sounds
            .iter()
            .filter(|command| **command == SoundCmd::PlaySe(PLAYER_LASER_SOUND))
            .count();
        native_laser_sound_count += native_laser_events;
        if let Some(retail_events) = retail_sound_events_for_update {
            let retail_laser_events = retail_events
                .iter()
                .filter(|event| **event == PLAYER_LASER_SOUND)
                .count();
            retail_laser_sound_count += retail_laser_events;
            assert_eq!(
                native_laser_events, retail_laser_events,
                "player-laser sound events at tick {tick}"
            );
        }

        if tick < FIRST_LEVEL_STATE_TICK {
            continue;
        }
        assert_eq!(
            native.game.vars.gameframe, retail_level_frame,
            "game frame at tick {tick}"
        );

        let retail_lasers = retail_lasers(&retail);
        let native_lasers = native_lasers(&native);
        if !retail_lasers.is_empty() || !native_lasers.is_empty() {
            first_weapon_tick.get_or_insert(tick);
            assert_eq!(native_lasers, retail_lasers, "laser state at tick {tick}");
            let native_view = Position(
                (native.frame().camera.x >> 16) as i16,
                (native.frame().camera.y >> 16) as i16,
                (native.frame().camera.z >> 16) as i16,
            );
            assert_eq!(
                native_view,
                retail_view_for_update.expect("aligned retail view snapshot"),
                "camera position at tick {tick}"
            );
            let native_draws = native_laser_draws(&native);
            let retail_draws = retail_draws_for_update
                .clone()
                .expect("aligned retail draw snapshot");
            if native_draws != retail_draws {
                first_draw_divergence.get_or_insert((tick, native_draws, retail_draws));
            }
            certified_weapon_updates += 1;
        }
    }

    let first_weapon_tick = first_weapon_tick.expect("script did not fire a player laser");
    assert_eq!(
        first_weapon_tick,
        FIRE_START_TICK + 2,
        "first firing update"
    );
    assert_eq!(
        native_laser_sound_count, 1,
        "native laser sound event count"
    );
    assert_eq!(
        retail_laser_sound_count, 1,
        "retail laser sound event count"
    );
    assert_eq!(
        first_draw_divergence, None,
        "first laser draw-command divergence"
    );
    if let Some((tick, game_frame, native_count, retail_count, differences)) =
        first_scene_draw_divergence.as_ref()
    {
        println!(
            "scene_draws first_divergence_tick={tick} game_frame={game_frame} native_count={native_count} retail_count={retail_count} differing_entries={} differences={differences:#?}",
            differences.len(),
        );
    } else {
        println!(
            "scene_draws certified_updates={sampled_internal_video_updates} first_divergence=none"
        );
    }
    assert_eq!(
        first_scene_draw_divergence, None,
        "authoritative retail scene draw list diverged"
    );
    match &first_internal_video_divergence {
        Some(divergence) => println!(
            "source_video_gate sampled_updates={} first_divergence={} retail_video_frame={} differing_pixels={} first_position={},{} retail_color={:?} native_color={:?} status=strict",
            sampled_internal_video_updates,
            divergence.sequence,
            divergence.retail_video_frame,
            divergence.differing_pixels,
            divergence.first_position[0],
            divergence.first_position[1],
            divergence.retail_color,
            divergence.native_color,
        ),
        None => println!(
            "source_video_gate sampled_updates={sampled_internal_video_updates} first_divergence=none status=strict"
        ),
    }
    assert_eq!(
        sampled_internal_video_updates,
        u32::from(source_video_last_game_frame - source_video_first_game_frame + 1),
        "authoritative source video duration"
    );
    if let Some((
        game_frame,
        compared,
        differing,
        (x, y, retail_index, native_index, owner, face),
        owner_draw,
        player_projection,
        (camera_position, camera_rotation),
    )) = &first_bitmap_divergence
    {
        println!(
            "source_bitmap_diagnostic first_divergence_game_frame={game_frame} compared_pixels={compared} differing_pixels={differing} first_position={x},{y} retail_index={retail_index} native_index={native_index} owner={owner} face={face} draw={owner_draw:?} player_projection={player_projection:?} camera_position={camera_position:?} camera_rotation={camera_rotation:?} status=non_comparable_typed_layer_decomposition"
        );
    } else {
        println!(
            "source_bitmap_diagnostic sampled_updates={sampled_bitmap_updates} first_divergence=none status=non_comparable_typed_layer_decomposition"
        );
    }
    assert_eq!(
        sampled_bitmap_updates, sampled_internal_video_updates,
        "source bitmap diagnostic duration"
    );
    assert_eq!(
        first_internal_video_divergence, None,
        "authoritative composed retail/native source video diverged"
    );
    assert!(
        pending_native_video.is_none(),
        "uncertified native video frame"
    );
    assert!(
        pending_source_video.is_empty(),
        "unmatched completed retail raster"
    );
    println!(
        "sf1_weapon certified_updates={certified_weapon_updates} first_divergence=none first_weapon_tick={first_weapon_tick} sound_events={native_laser_sound_count} retail_presentation_cadence={:?} native_gameplay_cadence={} source_coverage=playerfire,fire_elaser,pelaser",
        retail_frame_rate_range.expect("retail presentation cadence coverage"),
        native.game.vars.strategy.frame_rate,
    );
}
