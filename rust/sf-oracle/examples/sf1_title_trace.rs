//! Trace the retail and native title-demo object at the shared 20 Hz cadence.
//!
//! This is oracle archaeology: source addresses and the retail draw-list
//! layout deliberately remain outside the shipping Rust port.

use sf_core::pad;
use sf_game::shell::{GameState, Shell};
use sf_oracle::{
    load_retail_rom, RetailMachine, AL_ROTX, AL_ROTY, AL_ROTZ, AL_SFLAGS, AL_SFLAGS2, AL_SFLAGS3,
    AL_STRATPTR, AL_VX, AL_VY, AL_VZ, RETAIL_CURRENTBG, RETAIL_DOSTRATS, RETAIL_GAMEFRAME,
    RETAIL_MAPBANK, RETAIL_MAPCNT, RETAIL_MAPPTR, RETAIL_PLAYPT, RETAIL_POOL, RETAIL_PVIEWPOSZ,
    RETAIL_PVIEWVELZ, RETAIL_SHAPES, RETAIL_STAYBLACK, RETAIL_VIEW_POSITION_X,
    RETAIL_VIEW_POSITION_Y, RETAIL_VIEW_POSITION_Z,
};
use sf_render::draw_list::DrawListEntry as RenderDrawListEntry;
use sf_render::renderer::{
    config_from_repo_root, FrameInputs, GameState as RenderGameState, Renderer, WindowState,
    WINDOWARRAY_SIZE,
};
use sf_strat::common::{sv, StratRam};

const WORK_RAM: u32 = 0x7E_0000;
const ROM_WINDOW: u16 = 0x8000;
const RETAIL_TITLE_BACKGROUND: u16 = 249;
const RETAIL_DOTS_FLAG: u32 = 0x16F9;
const RETAIL_GRAPHICS_DOTS_MODE: usize = 0x0172;
const RETAIL_GRAPHICS_RANDOM_STATE: usize = 0x0140;
const RETAIL_GRAPHICS_DUST_POINTS: usize = 0x0B52;
const RETAIL_GRAPHICS_ROTATE_PACKED_ENTRY_BANK: u8 = 1;
const RETAIL_GRAPHICS_ROTATE_PACKED_ENTRY: u16 = 0x8938;
const RETAIL_GRAPHICS_OBJECT_MATRIX: usize = 0x0116;
const RETAIL_WINDOW_ARRAY: u32 = 0x1481;
const RETAIL_WINDOW_MODE: u32 = RETAIL_WINDOW_ARRAY + 64;
const RETAIL_BLACK_WINDOW_INDEX: u32 = 1;
const RETAIL_WINDOW_BYTES: u32 = 8;
const RETAIL_WINDOW_VALUE_OFFSET: u32 = 7;
const TITLE_DEMO_SHAPE: u16 = 225;
const SOURCE_SHAPE_COUNT: u16 = 512;
const VIDEO_FRAMES_PER_TICK: u32 = 3;
const SETUP_CONFIRM_CADENCE_TICKS: u32 = 60;
const SETUP_CONFIRM_HOLD_TICKS: u32 = 2;
const MAX_SETUP_TICKS: u32 = 240;
const TITLE_TRACE_TICKS: u32 = 96;
const MAX_VIDEO_FRAMES_PER_TITLE_UPDATE: u32 = 120;
const VIDEO_FRAMES_TO_PRESENT_UPDATE: u32 = 1;
const DEBUG_STATE_TICKS: u32 = 48;
const SOURCE_WIDTH: usize = 256;
const SOURCE_HEIGHT: usize = 224;
const MESEN_SCREEN_TOP: usize = 6;

const RETAIL_DRAW_COUNT: usize = 0x01B6;
const RETAIL_DRAW_LIST: usize = 0x0EF2;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Position(i16, i16, i16);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TitleDraw {
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

fn setup_input_for_tick(tick: u32) -> u16 {
    if tick % SETUP_CONFIRM_CADENCE_TICKS < SETUP_CONFIRM_HOLD_TICKS {
        pad::START
    } else {
        0
    }
}

fn retail_shape_id(machine: &RetailMachine, source_shape: u16) -> Option<u16> {
    (0..SOURCE_SHAPE_COUNT)
        .find(|shape_id| machine.peek16(RETAIL_SHAPES + u32::from(*shape_id) * 2) == source_shape)
}

fn retail_title_object(machine: &RetailMachine) -> Option<(u16, [u8; 3])> {
    machine.active_object_slots().into_iter().find_map(|slot| {
        let base = RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride;
        let source_shape = machine.peek16(WORK_RAM | base + RETAIL_POOL.al_shape);
        (retail_shape_id(machine, source_shape) == Some(TITLE_DEMO_SHAPE)).then(|| {
            (
                slot,
                [
                    machine.peek8(WORK_RAM | base + AL_ROTX),
                    machine.peek8(WORK_RAM | base + AL_ROTY),
                    machine.peek8(WORK_RAM | base + AL_ROTZ),
                ],
            )
        })
    })
}

fn retail_player_state(machine: &RetailMachine) -> (u32, [u8; 4], i16, i16) {
    let base = u32::from(machine.peek16(WORK_RAM | RETAIL_PLAYPT));
    let strategy = u32::from(machine.peek16(WORK_RAM | base + AL_STRATPTR))
        | (u32::from(machine.peek8(WORK_RAM | base + AL_STRATPTR + 2)) << 16);
    (
        strategy,
        [
            machine.peek8(WORK_RAM | base + AL_SFLAGS),
            machine.peek8(WORK_RAM | base + AL_SFLAGS2),
            machine.peek8(WORK_RAM | base + AL_SFLAGS3),
            machine.peek8(WORK_RAM | base + AL_SFLAGS3 + 1),
        ],
        machine.peek16(WORK_RAM | base + RETAIL_POOL.al_worldz) as i16,
        machine.peek16(WORK_RAM | base + AL_VZ) as i16,
    )
}

fn gsu_word(machine: &RetailMachine, address: usize) -> u16 {
    u16::from_le_bytes([
        machine.peek_gsu_ram(address),
        machine.peek_gsu_ram(address + 1),
    ])
}

fn retail_flat_shape(machine: &RetailMachine, source_shape: u16) -> u16 {
    retail_shape_id(machine, source_shape)
        .map(sf_core::shape::resolve_shape_word)
        .unwrap_or_else(|| sf_core::shape::resolve_shape_word(source_shape))
}

fn retail_draws(machine: &RetailMachine) -> Vec<TitleDraw> {
    let count = usize::from(gsu_word(machine, RETAIL_DRAW_COUNT));
    (0..count)
        .map(|list_order| {
            let base = RETAIL_DRAW_LIST + list_order * DRAW_ENTRY_BYTES;
            TitleDraw {
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

fn native_title_object(shell: &Shell) -> Option<(u16, [u8; 3])> {
    shell
        .game
        .objs
        .active_indices()
        .into_iter()
        .find_map(|slot| {
            let object = shell.game.objs.aliens[usize::from(slot)];
            (object.shape == TITLE_DEMO_SHAPE)
                .then_some((slot, [object.rotx, object.roty, object.rotz]))
        })
}

fn native_draws(shell: &Shell) -> Vec<TitleDraw> {
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
            TitleDraw {
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

fn hash_bytes(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xCBF2_9CE4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01B3)
    })
}

fn nonblack_pixels(rgb: &[u8]) -> usize {
    rgb.chunks_exact(3)
        .filter(|pixel| pixel.iter().any(|component| *component != 0))
        .count()
}

fn write_ppm(path: &str, rgb: &[u8]) {
    let mut ppm = format!("P6\n{SOURCE_WIDTH} {SOURCE_HEIGHT}\n255\n").into_bytes();
    ppm.extend_from_slice(rgb);
    std::fs::write(path, ppm).expect("write title trace image");
}

#[derive(Debug)]
struct VideoDivergence {
    tick: u32,
    retail_frame: u64,
    differing_pixels: usize,
    first_position: [usize; 2],
    retail_color: [u8; 3],
    native_color: [u8; 3],
}

fn mesen_frame_path(directory: &std::path::Path, source_game_frame: u16) -> std::path::PathBuf {
    let marker = format!("_game_{source_game_frame:03}_");
    let mut candidates: Vec<_> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read Mesen title directory {directory:?}: {error}"))
        .map(|entry| entry.expect("read Mesen title directory entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("sf1_title_frame_")
                        && name.contains(&marker)
                        && name.ends_with(".ppm")
                })
        })
        .collect();
    candidates.sort();
    assert_eq!(
        candidates.len(),
        3,
        "Mesen title game frame {source_game_frame} must have three presentation captures in {directory:?}"
    );
    candidates.pop().expect("three Mesen frame candidates")
}

fn read_mesen_source_rgb(directory: &std::path::Path, source_game_frame: u16) -> (u64, Vec<u8>) {
    let path = mesen_frame_path(directory, source_game_frame);
    let retail_frame = path
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.split('_').nth(3))
        .and_then(|field| field.parse::<u64>().ok())
        .expect("decimal Mesen retail frame in capture name");
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
    let mut newline_count = 0;
    let header_end = bytes
        .iter()
        .position(|byte| {
            if *byte == b'\n' {
                newline_count += 1;
            }
            newline_count == 3
        })
        .map(|position| position + 1)
        .expect("Mesen PPM has a three-line header");
    let header = std::str::from_utf8(&bytes[..header_end]).expect("Mesen PPM header is UTF-8");
    let mut fields = header.split_ascii_whitespace();
    assert_eq!(fields.next(), Some("P6"), "Mesen title capture format");
    let width = fields
        .next()
        .expect("Mesen PPM width")
        .parse::<usize>()
        .expect("decimal Mesen PPM width");
    let height = fields
        .next()
        .expect("Mesen PPM height")
        .parse::<usize>()
        .expect("decimal Mesen PPM height");
    assert_eq!(fields.next(), Some("255"), "Mesen title capture depth");
    assert_eq!(width, SOURCE_WIDTH, "Mesen title capture width");
    assert!(
        height >= MESEN_SCREEN_TOP + SOURCE_HEIGHT,
        "Mesen title capture height {height} is too short"
    );
    let pixels = &bytes[header_end..];
    assert_eq!(pixels.len(), width * height * 3, "Mesen PPM payload");
    let mut source = Vec::with_capacity(SOURCE_WIDTH * SOURCE_HEIGHT * 3);
    for row in MESEN_SCREEN_TOP..MESEN_SCREEN_TOP + SOURCE_HEIGHT {
        let start = row * SOURCE_WIDTH * 3;
        source.extend_from_slice(&pixels[start..start + SOURCE_WIDTH * 3]);
    }
    (retail_frame, source)
}

fn video_divergence(
    tick: u32,
    retail_frame: u64,
    retail: &[u8],
    native: &[u8],
) -> Option<VideoDivergence> {
    assert_eq!(retail.len(), SOURCE_WIDTH * SOURCE_HEIGHT * 3);
    assert_eq!(native.len(), retail.len());
    let mut differing_pixels = 0;
    let mut first = None;
    for (index, (expected, actual)) in retail
        .chunks_exact(3)
        .zip(native.chunks_exact(3))
        .enumerate()
    {
        if expected == actual {
            continue;
        }
        differing_pixels += 1;
        first.get_or_insert((
            index,
            [expected[0], expected[1], expected[2]],
            [actual[0], actual[1], actual[2]],
        ));
    }
    first.map(|(index, retail_color, native_color)| VideoDivergence {
        tick,
        retail_frame,
        differing_pixels,
        first_position: [index % SOURCE_WIDTH, index / SOURCE_WIDTH],
        retail_color,
        native_color,
    })
}

fn retail_video(machine: &RetailMachine) -> (u64, usize, [u8; 0x40], Vec<u8>) {
    let frame = machine.ppu_frame();
    let rgb: Vec<_> = frame
        .rgba
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().copied())
        .collect();
    (
        hash_bytes(&rgb),
        nonblack_pixels(&rgb),
        frame.registers,
        rgb,
    )
}

fn to_render_entry(entry: &sf_core::DrawListEntry) -> RenderDrawListEntry {
    RenderDrawListEntry {
        x: entry.x,
        y: entry.y,
        z: entry.z,
        rx: entry.rx,
        ry: entry.ry,
        rz: entry.rz,
        shape_id: entry.shape_id,
        color_table: entry.color_table,
        sort_z: entry.sort_z,
        sflags: entry.sflags,
        explosion_cnt: entry.explosion_cnt,
        anim_frame: entry.anim_frame,
        col_frame: entry.col_frame,
        depth_offset: entry.depth_offset,
        flags: entry.flags,
        shad_x: entry.shad_x,
        shad_y: entry.shad_y,
        shad_z: entry.shad_z,
        tscroll_x: entry.tscroll_x,
        tscroll_y: entry.tscroll_y,
        obj_id: entry.obj_id,
    }
}

fn native_video_hash(
    shell: &Shell,
    previous_draw_list: &[RenderDrawListEntry],
    renderer: &mut Renderer,
    include_scene_objects: bool,
    render_alpha: f32,
) -> (u64, usize, Vec<RenderDrawListEntry>, Vec<u8>) {
    let frame = shell.frame();
    let mut windows = [WindowState::default(); WINDOWARRAY_SIZE];
    for (destination, source) in windows.iter_mut().zip(frame.windows) {
        *destination = WindowState {
            mode: source.mode,
            wm_val: source.wm_val,
            stayblack: source.stayblack,
        };
    }
    let inputs = FrameInputs {
        game_state: RenderGameState::Title,
        currentbg: frame.currentbg,
        newmap: frame.newmap,
        bgflags: frame.bgflags,
        bg2_xscroll: frame.bg2_xscroll,
        nomax_bg2_yscroll: frame.nomax_bg2_yscroll,
        scene_style: frame.scene_style,
        point_pixels: &frame.point_pixels,
        pal_target: frame.pal_target,
        palfade_num: frame.palfade_num,
        windowmode: frame.windowmode,
        windows,
        display_brightness: frame.display_brightness,
        display_forced_blank: frame.display_forced_blank,
        display_black_subtraction: frame.display_black_subtraction,
        screen_wipe: frame.screen_wipe,
        screen_fill_circle: frame.screen_fill_circle,
        gameframe: frame.gameframe,
        ..FrameInputs::default()
    };
    let draw_list: Vec<_> = shell.draw_list().iter().map(to_render_entry).collect();
    let camera = frame.camera;
    renderer
        .transform
        .set_camera_fine(camera.x, camera.y, camera.z, camera.rotation);
    if camera.snap {
        renderer.transform.snap_camera();
    }
    renderer.begin_frame();
    // sf-app renders immediately after a fixed update with alpha zero, so the
    // just-completed list is the interpolation destination and the preceding
    // presented list remains visible at this exact boundary.
    let (rendered_previous, rendered_current) = if include_scene_objects {
        (previous_draw_list, draw_list.as_slice())
    } else {
        (&[][..], &[][..])
    };
    renderer.submit(rendered_previous, rendered_current, render_alpha, &inputs);
    renderer.end_frame();
    let rgb = renderer.read_pixels_rgb();
    (hash_bytes(&rgb), nonblack_pixels(&rgb), draw_list, rgb)
}

fn advance_retail_to_title(retail: &mut RetailMachine) -> u32 {
    for tick in 0..MAX_SETUP_TICKS {
        retail
            .tick_video_frames(setup_input_for_tick(tick), VIDEO_FRAMES_PER_TICK)
            .expect("retail title setup");
        if retail.peek16(WORK_RAM | RETAIL_CURRENTBG) == RETAIL_TITLE_BACKGROUND {
            return tick + 1;
        }
    }
    panic!("retail did not reach the title boundary");
}

fn advance_native_to_title(native: &mut Shell) -> u32 {
    for tick in 0..MAX_SETUP_TICKS {
        native.tick(setup_input_for_tick(tick));
        if native.state() == GameState::Title {
            return tick + 1;
        }
    }
    panic!("native did not reach the title boundary");
}

fn main() {
    let rom = load_retail_rom().expect("Star Fox retail ROM is required");
    let mut retail = RetailMachine::new(rom);
    let mut native = configured_shell();
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root");
    let mut renderer = Renderer::new_headless(256, 224, &config_from_repo_root(repo_root))
        .expect("headless title renderer");
    let retail_setup_ticks = advance_retail_to_title(&mut retail);
    let native_setup_ticks = advance_native_to_title(&mut native);
    assert_eq!(
        native_setup_ticks, retail_setup_ticks,
        "title setup duration diverged"
    );
    assert!(
        retail
            .tick_until_cpu_execution(0, RETAIL_DOSTRATS, MAX_VIDEO_FRAMES_PER_TITLE_UPDATE)
            .expect("retail initial title boundary"),
        "retail did not reach the first title strategy boundary"
    );
    let mut first_embedded_video_divergence = None;
    let mut first_authoritative_video_divergence = None;
    let mesen_directory: Option<std::path::PathBuf> =
        std::env::var_os("SF1_TITLE_MESEN_DIR").map(Into::into);
    let mut previous_native_draw_list = Vec::new();
    let debug_video = std::env::var_os("SF_TITLE_TRACE_DEBUG").is_some();
    let debug_dump_tick = std::env::var("SF_TITLE_TRACE_DUMP_TICK")
        .ok()
        .map(|value| value.parse::<u32>().expect("decimal title dump tick"));
    let debug_dump_directory: Option<std::path::PathBuf> =
        std::env::var_os("SF_TITLE_TRACE_DUMP_DIR").map(Into::into);
    if let Some(directory) = debug_dump_directory.as_ref() {
        std::fs::create_dir_all(directory).expect("create title trace dump directory");
    }
    let dump_only = std::env::var_os("SF_TITLE_TRACE_DUMP_ONLY").is_some();
    let render_alpha = std::env::var("SF_TITLE_TRACE_RENDER_ALPHA")
        .ok()
        .map(|value| value.parse::<f32>().expect("title render alpha"))
        .unwrap_or(0.0);

    println!("setup retail_ticks={retail_setup_ticks} native_ticks={native_setup_ticks}");
    for tick in 0..TITLE_TRACE_TICKS {
        let input = 0;
        if debug_dump_tick == Some(tick) {
            retail.watch_gsu_execution(
                RETAIL_GRAPHICS_ROTATE_PACKED_ENTRY_BANK,
                RETAIL_GRAPHICS_ROTATE_PACKED_ENTRY,
            );
        }
        assert!(
            retail
                .tick_until_cpu_execution(
                    input,
                    RETAIL_DOSTRATS,
                    MAX_VIDEO_FRAMES_PER_TITLE_UPDATE,
                )
                .expect("retail title trace"),
            "retail title update {tick} did not reach its next strategy boundary"
        );
        native.tick(input);

        let retail_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let native_frame = native.game.vars.gameframe;
        assert_eq!(retail_frame, native_frame, "game frame at trace {tick}");
        assert_eq!(
            retail.peek16(WORK_RAM | RETAIL_MAPCNT),
            native.game.vars.mapcnt,
            "map countdown at trace {tick}"
        );
        assert_eq!(native.state(), GameState::Title, "native state at {tick}");
        assert_eq!(
            retail.peek16(WORK_RAM | RETAIL_CURRENTBG),
            RETAIL_TITLE_BACKGROUND,
            "retail background at trace {tick}"
        );
        assert_eq!(
            retail.peek8(WORK_RAM | RETAIL_STAYBLACK) as i8,
            native.frame().stayblack,
            "black hold at trace {tick}"
        );
        let native_player_view_depth = native.game.vars.sv_i16(sv::PVIEWPOSZ);
        let retail_player_view_depth = retail.peek16(WORK_RAM | RETAIL_PVIEWPOSZ) as i16;
        let native_camera = native.frame().camera;
        let native_camera_position = Position(
            (native_camera.x >> 16) as i16,
            (native_camera.y >> 16) as i16,
            (native_camera.z >> 16) as i16,
        );
        let retail_camera_position = Position(
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_X) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Y) as i16,
            retail.peek16(WORK_RAM | RETAIL_VIEW_POSITION_Z) as i16,
        );
        let retail_pullback = retail_player_view_depth.wrapping_sub(retail_camera_position.2);
        if debug_video {
            if tick < DEBUG_STATE_TICKS {
                let (retail_strategy, retail_flags, retail_world_z, retail_velocity_z) =
                    retail_player_state(&retail);
                let native_player = native
                    .game
                    .objs
                    .player()
                    .expect("native title player exists");
                println!(
                    "player tick={tick} strategy={:?}/{retail_strategy:06X} flags={:?}/{retail_flags:?} \
                     world_z={}/{} object_velocity_z={}/{}",
                    native_player.stratptr,
                    [
                        native_player.sflags,
                        native_player.sflags2,
                        native_player.sflags3,
                        native_player.sflags4,
                    ],
                    native_player.worldz,
                    retail_world_z,
                    native_player.vz,
                    retail_velocity_z,
                );
            }
        } else {
            assert_eq!(
                native_player_view_depth,
                retail_player_view_depth,
                "title player-view depth at trace {tick}; native velocity={}, retail velocity={}",
                native.game.vars.pviewvelz,
                retail.peek16(WORK_RAM | RETAIL_PVIEWVELZ) as i16,
            );
            assert_eq!(
                native_camera_position,
                retail_camera_position,
                "title camera position at trace {tick}; native pullback={}, retail inferred pullback={retail_pullback}",
                native.game.vars.strategy.view_distance,
            );
        }
        if debug_video && tick < DEBUG_STATE_TICKS {
            println!(
                "camera tick={tick} player_view_z={native_player_view_depth}/{retail_player_view_depth} \
                 velocity={}/{} pullback={}/{} camera={native_camera_position:?}/{retail_camera_position:?}",
                native.game.vars.pviewvelz,
                retail.peek16(WORK_RAM | RETAIL_PVIEWVELZ) as i16,
                native.game.vars.strategy.view_distance,
                retail_pullback,
            );
            let retail_black_value = retail.peek8(
                WORK_RAM
                    | (RETAIL_WINDOW_ARRAY
                        + RETAIL_BLACK_WINDOW_INDEX * RETAIL_WINDOW_BYTES
                        + RETAIL_WINDOW_VALUE_OFFSET),
            );
            let native_black_value = native
                .frame()
                .windows
                .into_iter()
                .find(|window| window.mode == sf_game::windows::WINDOW_MODE_BLACK)
                .map(|window| window.wm_val);
            println!("black_window tick={tick} value={native_black_value:?}/{retail_black_value}");
        }

        let retail_object = retail_title_object(&retail).map(|(_, rotation)| rotation);
        let native_object = native_title_object(&native).map(|(_, rotation)| rotation);
        assert_eq!(retail_object, native_object, "title object at trace {tick}");

        if debug_video && tick < 20 {
            if (10..=13).contains(&tick) {
                let retail_map_pointer = retail.peek16(WORK_RAM | RETAIL_MAPPTR);
                let retail_map_bank = retail.peek8(WORK_RAM | RETAIL_MAPBANK);
                let retail_map_address =
                    (u32::from(retail_map_bank) << 16) | u32::from(retail_map_pointer | ROM_WINDOW);
                let retail_map_bytes: Vec<_> = (0..14)
                    .map(|offset| retail.peek8(retail_map_address + offset))
                    .collect();
                println!(
                    "map tick={tick} pointer={retail_map_bank:02X}:{retail_map_pointer:04X} \
                     bytes={retail_map_bytes:02X?} native_pointer={}",
                    native.game.vars.mapptr,
                );
            }
            if let (Some((retail_slot, _)), Some((native_slot, _))) =
                (retail_title_object(&retail), native_title_object(&native))
            {
                let retail_base = RETAIL_POOL.base + u32::from(retail_slot) * RETAIL_POOL.stride;
                let native_title = native.game.objs.aliens[usize::from(native_slot)];
                println!(
                    "title_object tick={tick} slot={native_slot}/{retail_slot} position={:?}/{:?} \
                     velocity={:?}/{:?}",
                    Position(
                        native_title.worldx,
                        native_title.worldy,
                        native_title.worldz,
                    ),
                    Position(
                        retail.peek16(WORK_RAM | retail_base + RETAIL_POOL.al_worldx) as i16,
                        retail.peek16(WORK_RAM | retail_base + RETAIL_POOL.al_worldy) as i16,
                        retail.peek16(WORK_RAM | retail_base + RETAIL_POOL.al_worldz) as i16,
                    ),
                    Position(native_title.vx, native_title.vy, native_title.vz),
                    Position(
                        retail.peek16(WORK_RAM | retail_base + AL_VX) as i16,
                        retail.peek16(WORK_RAM | retail_base + AL_VY) as i16,
                        retail.peek16(WORK_RAM | retail_base + AL_VZ) as i16,
                    ),
                );
            }
        }

        if !debug_video {
            assert_eq!(
                native_draws(&native),
                retail_draws(&retail),
                "complete ordered title draw commands at trace {tick}"
            );
        }

        // The retail renderer publishes a completed source frame after the
        // strategy boundary. Advancing one video frame reaches that stable
        // presentation without executing the following strategy update. An
        // independent Mesen capture verifies that the first visible title
        // image carries game-frame 15 while displaying the game-frame 14
        // projection; this native snapshot is the matching game-frame 14
        // render destination.
        retail
            .tick_video_frames(input, VIDEO_FRAMES_TO_PRESENT_UPDATE)
            .expect("retail title presentation");

        let (retail_video, retail_nonblack, retail_registers, retail_rgb) = retail_video(&retail);
        let (native_video, native_nonblack, current_native_draw_list, native_rgb) =
            native_video_hash(
                &native,
                &previous_native_draw_list,
                &mut renderer,
                true,
                render_alpha,
            );
        previous_native_draw_list = current_native_draw_list;
        if let Some(directory) = debug_dump_directory.as_ref() {
            write_ppm(
                directory
                    .join(format!("retail-{tick:03}.ppm"))
                    .to_str()
                    .expect("title trace dump path is UTF-8"),
                &retail_rgb,
            );
            write_ppm(
                directory
                    .join(format!("native-{tick:03}.ppm"))
                    .to_str()
                    .expect("title trace dump path is UTF-8"),
                &native_rgb,
            );
        }
        if debug_dump_tick == Some(tick) {
            println!("dump_native_draws={:?}", native.draw_list());
            println!("dump_native_semantic_draws={:?}", native_draws(&native));
            println!("dump_retail_semantic_draws={:?}", retail_draws(&retail));
            println!("dump_native_camera={:?}", native.frame().camera);
            println!(
                "dump_native_display brightness={} forced_blank={} black_subtraction={}",
                native.frame().display_brightness,
                native.frame().display_forced_blank,
                native.frame().display_black_subtraction,
            );
            println!(
                "dump_native_point_state slow={} depth={} pixel_count={} pixels={:?}",
                native.game.vars.space_dust_uses_reduced_speed,
                native.game.vars.space_dust_view_depth,
                native.frame().point_pixels.len(),
                native.frame().point_pixels,
            );
            println!(
                "dump_dots native={} retail={} graphics={} random={} first_points={:?}",
                native.game.vars.dotsflag,
                retail.peek16(WORK_RAM | RETAIL_DOTS_FLAG) as i16,
                gsu_word(&retail, RETAIL_GRAPHICS_DOTS_MODE) as i16,
                gsu_word(&retail, RETAIL_GRAPHICS_RANDOM_STATE),
                (0..6)
                    .map(|offset| gsu_word(&retail, RETAIL_GRAPHICS_DUST_POINTS + offset * 2))
                    .collect::<Vec<_>>(),
            );
            println!(
                "dump_retail_object_matrix bytes={:02X?} rotate_watch={:?}",
                (0..10)
                    .map(|offset| { retail.peek_gsu_ram(RETAIL_GRAPHICS_OBJECT_MATRIX + offset) })
                    .collect::<Vec<_>>(),
                retail.gsu_execution_watch_state(),
            );
            let retail_ppu = retail.ppu_frame();
            println!(
                "dump_retail_scroll hofs={:?} vofs={:?}",
                retail_ppu.bg_hofs, retail_ppu.bg_vofs
            );
            println!(
                "dump_retail_windows mode={} bytes={:02X?}",
                retail.peek8(WORK_RAM | RETAIL_WINDOW_MODE),
                (0..65)
                    .map(|offset| retail.peek8(WORK_RAM | RETAIL_WINDOW_ARRAY + offset))
                    .collect::<Vec<_>>(),
            );
            write_ppm("/tmp/starfox-title-retail.ppm", &retail_rgb);
            let retail_snapshot_rgb: Vec<_> = retail
                .ppu_snapshot_rgba()
                .chunks_exact(4)
                .flat_map(|pixel| pixel[..3].iter().copied())
                .collect();
            write_ppm(
                "/tmp/starfox-title-retail-snapshot.ppm",
                &retail_snapshot_rgb,
            );
            for bg in 0..3 {
                let retail_bg_rgb: Vec<_> = retail
                    .ppu_snapshot_bg_rgba(bg)
                    .chunks_exact(4)
                    .flat_map(|pixel| pixel[..3].iter().copied())
                    .collect();
                write_ppm(
                    &format!("/tmp/starfox-title-retail-bg{}.ppm", bg + 1),
                    &retail_bg_rgb,
                );
                std::fs::write(
                    format!("/tmp/starfox-title-retail-bg{}-indices.bin", bg + 1),
                    retail.ppu_snapshot_bg_indices(bg),
                )
                .expect("write title trace background indices");
            }
            write_ppm("/tmp/starfox-title-native.ppm", &native_rgb);
            std::fs::write("/tmp/starfox-title-retail.cgram", retail_ppu.cgram)
                .expect("write title trace palette");
            std::fs::write("/tmp/starfox-title-retail.vram", retail_ppu.vram)
                .expect("write title trace video data");
            std::fs::write("/tmp/starfox-title-retail.ppu-state", retail_ppu.registers)
                .expect("write title trace display state");
            let graphics_ram: Vec<_> = (0..32_768)
                .map(|address| retail.peek_gsu_ram(address))
                .collect();
            std::fs::write("/tmp/starfox-title-retail.graphics-state", graphics_ram)
                .expect("write title trace graphics state");
            let (_, _, _, native_background_rgb) =
                native_video_hash(&native, &[], &mut renderer, false, render_alpha);
            write_ppm(
                "/tmp/starfox-title-native-background.ppm",
                &native_background_rgb,
            );
            if dump_only {
                println!("diagnostic_dump completed_tick={tick}");
                return;
            }
        }
        if debug_video && tick < DEBUG_STATE_TICKS {
            let frame = native.frame();
            println!(
                "video tick={tick} retail_frame={} retail_nonblack={retail_nonblack} \
                 inidisp={} main_screen={} native_nonblack={native_nonblack} \
                 windowmode={} windows={:?}",
                retail.video_frame(),
                retail_registers[0],
                retail_registers[0x2C],
                frame.windowmode,
                frame.windows,
            );
        }
        if retail_video != native_video && first_embedded_video_divergence.is_none() {
            first_embedded_video_divergence =
                Some((tick, retail.video_frame(), retail_video, native_video));
        }
        if let Some(directory) = mesen_directory.as_deref() {
            let source_game_frame = native_frame.wrapping_add(1);
            let (retail_frame, authoritative_rgb) =
                read_mesen_source_rgb(directory, source_game_frame);
            if first_authoritative_video_divergence.is_none() {
                first_authoritative_video_divergence =
                    video_divergence(tick, retail_frame, &authoritative_rgb, &native_rgb);
            }
        }
    }

    println!(
        "semantic/object/draw certified_updates={TITLE_TRACE_TICKS} first_divergence=none final_retail_video_frame={}",
        retail.video_frame(),
    );
    match first_embedded_video_divergence {
        Some((tick, retail_frame, retail_hash, native_hash)) => println!(
            "embedded_video_diagnostic first_divergence={tick} retail_video_frame={retail_frame} \
             retail_hash={retail_hash} native_hash={native_hash}"
        ),
        None => println!("embedded_video_diagnostic matching_updates={TITLE_TRACE_TICKS}"),
    }
    if mesen_directory.is_some() {
        match &first_authoritative_video_divergence {
            Some(divergence) => println!(
                "source_resolution_video certified_updates={} first_divergence={} retail_video_frame={} \
                 differing_pixels={} first_position={},{} retail_color={:?} native_color={:?}",
                divergence.tick,
                divergence.tick,
                divergence.retail_frame,
                divergence.differing_pixels,
                divergence.first_position[0],
                divergence.first_position[1],
                divergence.retail_color,
                divergence.native_color,
            ),
            None => println!(
                "source_resolution_video certified_updates={TITLE_TRACE_TICKS} first_divergence=none"
            ),
        }
        assert!(
            first_authoritative_video_divergence.is_none(),
            "authoritative Mesen title video diverged"
        );
    }
}
