//! Trace the retail and native title-demo object at the shared 20 Hz cadence.
//!
//! This is oracle archaeology: source addresses and the retail draw-list
//! layout deliberately remain outside the shipping Rust port.

use sf_core::pad;
use sf_game::shell::{GameState, Shell};
use sf_oracle::{
    load_retail_rom, RetailMachine, AL_ROTX, AL_ROTY, AL_ROTZ, RETAIL_CURRENTBG, RETAIL_DOSTRATS,
    RETAIL_GAMEFRAME, RETAIL_MAPCNT, RETAIL_POOL, RETAIL_SHAPES, RETAIL_STAYBLACK,
};
use sf_render::draw_list::DrawListEntry as RenderDrawListEntry;
use sf_render::renderer::{
    config_from_repo_root, FrameInputs, GameState as RenderGameState, Renderer, WindowState,
    WINDOWARRAY_SIZE,
};

const WORK_RAM: u32 = 0x7E_0000;
const RETAIL_TITLE_BACKGROUND: u16 = 249;
const TITLE_DEMO_SHAPE: u16 = 225;
const SOURCE_SHAPE_COUNT: u16 = 512;
const VIDEO_FRAMES_PER_TICK: u32 = 3;
const SETUP_CONFIRM_CADENCE_TICKS: u32 = 60;
const SETUP_CONFIRM_HOLD_TICKS: u32 = 2;
const MAX_SETUP_TICKS: u32 = 240;
const TITLE_TRACE_TICKS: u32 = 96;
const MAX_VIDEO_FRAMES_PER_TITLE_UPDATE: u32 = 120;
const VIDEO_FRAMES_TO_PRESENT_UPDATE: u32 = 1;

/// Retail Rev 2 GSU-RAM title draw-list base, located independently by the
/// title shape pointer and verified across every captured roll update.
const RETAIL_TITLE_DRAW_LIST: usize = 0x0EF2;
const DRAW_ROTATION_X: usize = 4;
const DRAW_ROTATION_Y: usize = 5;
const DRAW_ROTATION_Z: usize = 6;
const DRAW_SHAPE: usize = 8;

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

fn retail_title_draw(machine: &RetailMachine) -> Option<(usize, [u8; 3])> {
    let title_shape = machine.peek16(RETAIL_SHAPES + u32::from(TITLE_DEMO_SHAPE) * 2);
    let source_shape = u16::from_le_bytes([
        machine.peek_gsu_ram(RETAIL_TITLE_DRAW_LIST + DRAW_SHAPE),
        machine.peek_gsu_ram(RETAIL_TITLE_DRAW_LIST + DRAW_SHAPE + 1),
    ]);
    (source_shape == title_shape).then(|| {
        (
            0,
            [
                machine.peek_gsu_ram(RETAIL_TITLE_DRAW_LIST + DRAW_ROTATION_X),
                machine.peek_gsu_ram(RETAIL_TITLE_DRAW_LIST + DRAW_ROTATION_Y),
                machine.peek_gsu_ram(RETAIL_TITLE_DRAW_LIST + DRAW_ROTATION_Z),
            ],
        )
    })
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

fn native_title_draw(shell: &Shell) -> Option<(usize, [i16; 3])> {
    shell
        .draw_list()
        .iter()
        .enumerate()
        .find(|(_, entry)| entry.shape_id == TITLE_DEMO_SHAPE)
        .map(|(entry, draw)| (entry, [draw.rx, draw.ry, draw.rz]))
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
    let mut ppm = format!("P6\n256 224\n255\n").into_bytes();
    ppm.extend_from_slice(rgb);
    std::fs::write(path, ppm).expect("write title trace image");
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
        pal_target: frame.pal_target,
        palfade_num: frame.palfade_num,
        windowmode: frame.windowmode,
        windows,
        screen_wipe: frame.screen_wipe,
        screen_fill_circle: frame.screen_fill_circle,
        gameframe: frame.gameframe,
        ..FrameInputs::default()
    };
    let draw_list: Vec<_> = shell.draw_list().iter().map(to_render_entry).collect();
    let camera = frame.camera;
    renderer.transform.set_camera(
        camera.x, camera.y, camera.z, camera.rx, camera.ry, camera.rz,
    );
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
    renderer.submit(rendered_previous, rendered_current, 0.0, &inputs);
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
    let mut first_video_divergence = None;
    let mut previous_native_draw_list = Vec::new();
    let debug_video = std::env::var_os("SF_TITLE_TRACE_DEBUG").is_some();
    let debug_dump_tick = std::env::var("SF_TITLE_TRACE_DUMP_TICK")
        .ok()
        .map(|value| value.parse::<u32>().expect("decimal title dump tick"));

    println!("setup retail_ticks={retail_setup_ticks} native_ticks={native_setup_ticks}");
    for tick in 0..TITLE_TRACE_TICKS {
        let input = 0;
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

        let retail_object = retail_title_object(&retail).map(|(_, rotation)| rotation);
        let native_object = native_title_object(&native).map(|(_, rotation)| rotation);
        assert_eq!(retail_object, native_object, "title object at trace {tick}");

        let retail_draw =
            retail_title_draw(&retail).map(|(order, rotation)| (order, rotation.map(i16::from)));
        assert_eq!(
            retail_draw,
            native_title_draw(&native),
            "ordered title draw at trace {tick}"
        );

        retail
            .tick_video_frames(input, VIDEO_FRAMES_TO_PRESENT_UPDATE)
            .expect("retail title presentation");

        let (retail_video, retail_nonblack, retail_registers, retail_rgb) = retail_video(&retail);
        let (native_video, native_nonblack, current_native_draw_list, native_rgb) =
            native_video_hash(&native, &previous_native_draw_list, &mut renderer, true);
        previous_native_draw_list = current_native_draw_list;
        if debug_dump_tick == Some(tick) {
            write_ppm("/tmp/starfox-title-retail.ppm", &retail_rgb);
            write_ppm("/tmp/starfox-title-native.ppm", &native_rgb);
            let (_, _, _, native_background_rgb) =
                native_video_hash(&native, &[], &mut renderer, false);
            write_ppm(
                "/tmp/starfox-title-native-background.ppm",
                &native_background_rgb,
            );
        }
        if debug_video && tick < 24 {
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
        if retail_video != native_video && first_video_divergence.is_none() {
            first_video_divergence = Some((tick, retail.video_frame(), retail_video, native_video));
        }
    }

    println!("semantic/object/draw certified_updates={TITLE_TRACE_TICKS} first_divergence=none");
    match first_video_divergence {
        Some((tick, retail_frame, retail_hash, native_hash)) => println!(
            "source_resolution_video first_divergence={tick} retail_video_frame={retail_frame} \
             retail_hash={retail_hash} native_hash={native_hash}"
        ),
        None => println!("source_resolution_video certified_updates={TITLE_TRACE_TICKS}"),
    }
}
