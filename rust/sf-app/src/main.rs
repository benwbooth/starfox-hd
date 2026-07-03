//! Star Fox HD — Rust application shell (the RIIR integration binary).
//!
//! Port (C oracle): `src/main.c` — SDL init, GL 3.3 core context, fixed
//! 50 ms (20 Hz) tick accumulator with render interpolation, SF_STATE_DUMP.
//! Game-side state machine and frame-input producers live in
//! `sf_game::shell`; rendering in `sf_render`; audio in `sf_audio`.
//!
//! Extra env knobs (not in the C build, used by the smoke tests):
//! - `SF_MAX_TICKS=<n>`  exit after n game ticks
//! - `SF_HIDDEN=1`       create the window hidden (headless-ish CI runs)
//! - `SF_DUMP_PPM=<path>` write one RGB PPM frame readback (at tick
//!   `SF_DUMP_PPM_TICK`, default 220).

mod audio;
mod config;
mod input;
mod statedump;

use std::path::PathBuf;
use std::time::Instant;

use sdl3::event::{Event, WindowEvent};
use sdl3::keyboard::Keycode;
use sdl3::video::FullscreenType;

use sf_core::{DrawListEntry as CoreEntry, GAME_TICK_MS, MAX_DRAW_LIST};
use sf_game::shell::Shell;
use sf_render::draw_list::DrawListEntry as RenderEntry;
use sf_render::renderer::{
    FrameInputs, GameState as RenderState, Renderer, RendererConfig, WindowState,
    WINDOWARRAY_SIZE,
};

use crate::audio::AudioSys;
use crate::config::Config;
use crate::input::Input;
use crate::statedump::StateDump;

/// sf_core entry -> sf_render entry (field-identical structs; the render
/// crate keeps its own copy so it does not depend on game types).
fn to_render_entry(e: &CoreEntry) -> RenderEntry {
    RenderEntry {
        x: e.x,
        y: e.y,
        z: e.z,
        rx: e.rx,
        ry: e.ry,
        rz: e.rz,
        shape_id: e.shape_id,
        color_table: e.color_table,
        sort_z: e.sort_z,
        sflags: e.sflags,
        explosion_cnt: e.explosion_cnt,
        anim_frame: e.anim_frame,
        col_frame: e.col_frame,
        depth_offset: e.depth_offset,
        flags: e.flags,
        shad_x: e.shad_x,
        shad_y: e.shad_y,
        shad_z: e.shad_z,
        tscroll_x: e.tscroll_x,
        tscroll_y: e.tscroll_y,
        obj_id: e.obj_id,
    }
}

/// Shell state code -> sf_render GameState (same boot.h ordering).
fn to_render_state(code: u8) -> RenderState {
    match code {
        0 => RenderState::Boot,
        1 => RenderState::Title,
        2 => RenderState::Briefing,
        3 => RenderState::PlanetSelect,
        4 => RenderState::Playing,
        5 => RenderState::Continue,
        _ => RenderState::Ending,
    }
}

struct CliArgs {
    config_path: PathBuf,
    asset_root: Option<PathBuf>,
    shader_dir: Option<PathBuf>,
}

fn parse_args() -> CliArgs {
    let mut args = CliArgs {
        config_path: PathBuf::from("starfox.ini"),
        asset_root: None,
        shader_dir: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--config" => {
                if let Some(v) = it.next() {
                    args.config_path = PathBuf::from(v);
                }
            }
            "--asset-root" => args.asset_root = it.next().map(PathBuf::from),
            "--shader-dir" => args.shader_dir = it.next().map(PathBuf::from),
            other => eprintln!("Unknown argument: {other}"),
        }
    }
    args
}

fn write_ppm(path: &str, w: i32, h: i32, rgb: &[u8]) {
    let mut buf = format!("P6\n{w} {h}\n255\n").into_bytes();
    buf.extend_from_slice(rgb);
    if let Err(e) = std::fs::write(path, buf) {
        eprintln!("SF_DUMP_PPM: write {path} failed: {e}");
    } else {
        println!("SF_DUMP_PPM: wrote {path}");
    }
}

fn main() {
    let cli = parse_args();
    let cfg = Config::load(&cli.config_path);

    // --- SDL init (main.c Init) ---
    let sdl = match sdl3::init() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SDL_Init failed: {e}");
            std::process::exit(1);
        }
    };
    let video = sdl.video().expect("SDL video subsystem");

    let hidden = std::env::var("SF_HIDDEN").map(|v| v == "1").unwrap_or(false);
    let mut builder = video.window(
        "Star Fox HD",
        cfg.window_width as u32,
        cfg.window_height as u32,
    );
    builder.resizable().position_centered();
    if hidden {
        builder.hidden();
    }
    if cfg.fullscreen != 0 {
        // SDL3 collapsed exclusive/desktop fullscreen; both config values
        // map to fullscreen (main.c:43-44).
        builder.fullscreen();
    }
    let mut window = match builder.build() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("SDL_CreateWindow failed: {e}");
            std::process::exit(1);
        }
    };

    // wgpu renders into the SDL3 window's surface via raw-window-handle.
    // SAFETY: `window` outlives `gpu` (drop order is reverse of declaration),
    // so the 'static surface never dangles.
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let surface = match unsafe {
        instance.create_surface_unsafe(
            wgpu::SurfaceTargetUnsafe::from_window(&window)
                .expect("SDL3 window handle -> wgpu surface target"),
        )
    } {
        Ok(s) => s,
        Err(e) => {
            eprintln!("wgpu create_surface failed: {e}");
            std::process::exit(1);
        }
    };
    let gpu = match sf_render::gpu::Gpu::new_for_surface(
        instance,
        surface,
        cfg.window_width as u32,
        cfg.window_height as u32,
    ) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("wgpu init failed: {e}");
            std::process::exit(1);
        }
    };

    // --- Renderer / audio / input / game shell ---
    let render_cfg = RendererConfig {
        shader_dir: cli.shader_dir.clone(),
        asset_root: cli.asset_root.clone().unwrap_or_else(|| cfg.asset_root()),
    };
    let mut renderer = match Renderer::new(
        gpu,
        cfg.window_width,
        cfg.window_height,
        &render_cfg,
    ) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Renderer init failed: {e}");
            std::process::exit(1);
        }
    };

    let mut audio = AudioSys::new(&sdl, cfg.audio_asset_dir());

    let gamepad_subsystem = sdl.gamepad().ok();
    let mut input = Input::new();
    if let Some(gp) = &gamepad_subsystem {
        input.open_all_gamepads(gp);
    }

    let mut shell = Shell::new();
    // C boot.c: Strat_RegisterAll() after World_Init — populate g_istrats[]
    // and the strategy address map so map objects get their per-frame AI and
    // the player alien exists.
    // Install the strategy registration hook: runs now and re-runs after
    // every World::init() reset in the shell (C re-runs Strat_RegisterAll on
    // each level load; a startup-only call gets wiped by the first reset).
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    // Player spawn at gameplay start (C Strat_SpawnPlayer +
    // Strat_PlayerOpening_Init for the scramble levels, boot.c:89-102).
    shell.set_spawn_player(Box::new(|game, newmap| {
        if let Some(idx) = sf_strat::player::strat_spawn_player(game) {
            if newmap == sf_map::catalog::map_id::M1_1
                || newmap == sf_map::catalog::map_id::M2_1
                || newmap == sf_map::catalog::map_id::M3_1
            {
                sf_strat::player::strat_player_opening_init(game, idx);
            }
        }
    }));
    // Wire real per-shape collision half-extents (C load_collision_extents)
    // from the renderer's shape meshes into the collision system. The shape
    // store is fully populated by Renderer::new (register_builtins).
    shell.set_shape_extents(renderer.shapes.all_shape_half_extents());

    let mut dump = StateDump::from_env();
    let max_ticks: Option<u64> = std::env::var("SF_MAX_TICKS")
        .ok()
        .and_then(|v| v.parse().ok());
    let ppm_path = std::env::var("SF_DUMP_PPM").ok().filter(|p| !p.is_empty());
    let ppm_tick: u64 = std::env::var("SF_DUMP_PPM_TICK")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(220);
    let mut ppm_pending = ppm_path.is_some();
    // SF_NOINTERP=1: render the tick-exact game state (no render-frame
    // interpolation). Every frame then shows a true 20 Hz game state that
    // matches the ROM's per-tick output — authentic (choppier) and the
    // correct basis for parity checking, since interpolation invents
    // between-tick states the ROM never computes.
    let no_interp = std::env::var("SF_NOINTERP").map(|v| v == "1").unwrap_or(false);

    // --- Fixed timestep game loop with interpolation (main.c:153-201) ---
    let tick_duration = GAME_TICK_MS as f64 / 1000.0;
    let mut prev_time = Instant::now();
    let mut accumulator = 0.0f64;

    let mut prev_list: Vec<RenderEntry> = Vec::with_capacity(MAX_DRAW_LIST);
    let mut curr_list: Vec<RenderEntry> = Vec::with_capacity(MAX_DRAW_LIST);
    let mut total_ticks: u64 = 0;
    let mut running = true;
    let (mut fb_w, mut fb_h) = (cfg.window_width, cfg.window_height);

    let mut event_pump = sdl.event_pump().expect("event pump");

    while running {
        let now = Instant::now();
        let mut dt = now.duration_since(prev_time).as_secs_f64();
        prev_time = now;
        // Cap dt to prevent spiral of death (main.c:171).
        if dt > 0.25 {
            dt = 0.25;
        }
        accumulator += dt;

        // --- HandleEvents (main.c:86-111) ---
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => running = false,
                Event::Window {
                    win_event: WindowEvent::Resized(w, h),
                    ..
                }
                | Event::Window {
                    win_event: WindowEvent::PixelSizeChanged(w, h),
                    ..
                } => {
                    renderer.resize(w, h);
                    fb_w = w;
                    fb_h = h;
                }
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => running = false,
                Event::KeyDown {
                    keycode: Some(Keycode::F11),
                    ..
                } => {
                    let fs = window.fullscreen_state() != FullscreenType::Off;
                    let _ = window.set_fullscreen(!fs);
                }
                Event::ControllerDeviceAdded { which, .. } => {
                    if let Some(gp) = &gamepad_subsystem {
                        input.add_gamepad(gp, sdl3::sys::joystick::SDL_JoystickID(which));
                    }
                }
                Event::ControllerDeviceRemoved { which, .. } => {
                    input.remove_gamepad(sdl3::sys::joystick::SDL_JoystickID(which));
                }
                _ => {}
            }
        }

        // --- Fixed timestep game ticks (main.c:177-189) ---
        let mut frame = shell.frame();
        while accumulator >= tick_duration {
            prev_list.clear();
            prev_list.extend_from_slice(&curr_list);

            // SfRtl_BeginFrame -> Game_Tick -> Game_GetDrawList.
            input.begin_frame(&event_pump.keyboard_state());
            shell.tick(input.pad1);
            curr_list.clear();
            curr_list.extend(
                shell
                    .draw_list()
                    .iter()
                    .take(MAX_DRAW_LIST)
                    .map(to_render_entry),
            );

            frame = shell.frame();
            audio.tick(&mut shell, &frame);

            if let Some(d) = dump.as_mut() {
                let list = shell.draw_list();
                d.tick(
                    input.pad1,
                    frame.game_state_code,
                    &list[..list.len().min(MAX_DRAW_LIST)],
                );
            }

            // Camera for the render pass (nmi.c GameCamera_Update ->
            // Transform_SetCamera / Transform_SnapCamera).
            let cam = frame.camera;
            renderer
                .transform
                .set_camera(cam.x, cam.y, cam.z, cam.rx, cam.ry, cam.rz);
            if cam.snap {
                renderer.transform.snap_camera();
            }

            accumulator -= tick_duration;
            total_ticks += 1;
            if let Some(max) = max_ticks {
                if total_ticks >= max {
                    running = false;
                    accumulator = 0.0;
                    break;
                }
            }
        }

        // Interpolation alpha for smooth rendering (main.c:192). alpha=1
        // shows the latest tick verbatim (tick-exact / ROM-accurate).
        let alpha = if no_interp {
            1.0
        } else {
            (accumulator / tick_duration) as f32
        };

        // Assemble FrameInputs from the shell snapshot.
        let mut windows = [WindowState::default(); WINDOWARRAY_SIZE];
        for (dst, src) in windows.iter_mut().zip(frame.windows.iter()) {
            *dst = WindowState {
                mode: src.mode,
                wm_val: src.wm_val,
                stayblack: src.stayblack,
            };
        }
        let inputs = FrameInputs {
            game_state: to_render_state(frame.game_state_code),
            currentbg: frame.currentbg,
            newmap: frame.newmap,
            bgflags: frame.bgflags,
            bg2_xscroll: frame.bg2_xscroll,
            nomax_bg2_yscroll: frame.nomax_bg2_yscroll,
            windowmode: frame.windowmode,
            windows,
            meters: frame.meters,
            stayblack: frame.stayblack,
            gameflags: frame.gameflags,
            gameframe: frame.gameframe,
            boostcnt: frame.boostcnt,
            arrows: frame.arrows,
            splayerflymode: frame.splayerflymode,
            stage: frame.stage,
            shield_cur: frame.shield_cur,
            shield_max: frame.shield_max,
            boss_hp_cur: frame.boss_hp_cur,
            boss_hp_max: frame.boss_hp_max,
            lives: frame.lives,
            bombs: frame.bombs,
            msg_count1: frame.msg_count1,
            msg_count2: frame.msg_count2,
            whichfriend: frame.whichfriend,
            friends_meter: frame.friends_meter,
            message_text: frame.message_text.as_deref(),
            whichroute: frame.whichroute,
            currentplanet: frame.currentplanet,
            nebula_on: frame.nebula_on,
            route_path_ids: &frame.route_path_ids,
        };

        // Render interpolated frame (main.c:195-200).
        renderer.begin_frame();
        renderer.submit(&prev_list, &curr_list, alpha, &inputs);
        renderer.end_frame(); // uploads geometry + presents the wgpu surface

        if ppm_pending && total_ticks >= ppm_tick {
            if let Some(path) = &ppm_path {
                let rgb = renderer.read_pixels_rgb();
                write_ppm(path, fb_w, fb_h, &rgb);
            }
            ppm_pending = false;
        }
    }

    // Final PPM if the run ended before the requested tick.
    if ppm_pending {
        if let Some(path) = &ppm_path {
            let rgb = renderer.read_pixels_rgb();
            write_ppm(path, fb_w, fb_h, &rgb);
        }
    }

    if let Some(d) = dump.as_mut() {
        d.flush();
    }
    renderer.shutdown();
    println!("Shutdown after {total_ticks} ticks");
}
