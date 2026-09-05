#![allow(dead_code)]

use sf_core::pad;
use sf_game::shell::{FrameSnapshot, Shell};
use sf_oracle::{RetailMachine, RETAIL_SHAPES};
use sf_render::draw_list::{DrawListEntry as RenderDrawListEntry, SourceSceneCamera};
use sf_render::renderer::{
    FrameInputs, GameState as RenderGameState, Renderer, WindowState, WINDOWARRAY_SIZE,
};

pub const WEAPON_TRACE_END_TICK: u32 = 1_231;
const WEAPON_TRACE_END_TICK_ENV: &str = "SF1_WEAPON_TRACE_END_TICK";
pub const WEAPON_VIDEO_CAPTURE_FIRST_GAME_FRAME: u16 = 312;
pub const WEAPON_VIDEO_CAPTURE_LAST_GAME_FRAME: u16 = 337;
pub const WEAPON_VIDEO_PRESENTATION_LAST_GAME_FRAME: u16 = WEAPON_VIDEO_CAPTURE_LAST_GAME_FRAME + 1;

pub fn weapon_trace_end_tick() -> u32 {
    std::env::var(WEAPON_TRACE_END_TICK_ENV)
        .ok()
        .map(|value| {
            value
                .parse::<u32>()
                .expect("weapon trace end tick must be decimal")
        })
        .unwrap_or(WEAPON_TRACE_END_TICK)
}

const SOURCE_SHAPE_CATALOG_ENTRIES: u16 = 512;
const GSU_DRAW_COUNT: usize = 0x01B6;
const GSU_DRAW_HEAD: usize = 0x021E;
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
const DIRECT_SHAPE_IDS: [(u16, u16); 21] = [
    (0xDD84, sf_map::consts::sh::MYBASE_0),
    (0xB369, 511),
    (0xB219, sf_map::consts::sh::BOOST_SHAPE),
    (0xDD30, 298),
    (0xBD40, 482),
    (0xB34D, sf_render::shape_data::SHAPE_EXT_ELASER2A),
    (0xB075, 479),
    (0xB289, 367),
    (0xB2A5, 342),
    (0xB2C1, 380),
    (0xB11D, 462),
    (0xBE04, 466),
    (0xB101, 461),
    (0xB587, 465),
    (0xACF5, 2),
    (0xADD5, sf_render::shape_data::SHAPE_EXT_SMOKE),
    (0xAEED, sf_render::shape_data::SHAPE_EXT_BOUNCYBALL),
    (0xBB9C, 420),
    (0xBD78, sf_render::shape_data::SHAPE_EXT_TOW_1),
    (0xB882, sf_render::shape_data::SHAPE_EXT_PILLAR3_NS),
    (0xC360, 351),
];
const RETAIL_INTRO_SHAPE_IDS: [(u16, u16); 4] = [
    (0xBB48, sf_render::shapes::SHAPE_ALIAS_OP_0),
    (0xBB64, sf_render::shapes::SHAPE_ALIAS_OP_1),
    (0xBB80, sf_render::shapes::SHAPE_ALIAS_OP_2),
    (0xD304, sf_core::shape::SF1_SHAPE_INTRO_ARWING),
];

const FRONT_END_CONFIRM_CADENCE_TICKS: u32 = 60;
const FRONT_END_CONFIRM_HOLD_TICKS: u32 = 2;
const FRONT_END_LAST_CONFIRM_TICK: u32 = 360;
const GAME_DESTINATION_SELECT_TICK: u32 = 380;
const GAME_DESTINATION_CONFIRM_TICK: u32 = 420;
const ROUTE_SELECTION_CONFIRM_TICK: u32 = 500;
const ROUTE_SELECTION_CONFIRM_HOLD_TICKS: u32 = 12;
const PLANET_DISMISS_START_TICK: u32 = 840;
const PLANET_DISMISS_END_TICK: u32 = 900;
const PLANET_DISMISS_CADENCE_TICKS: u32 = 2;
const FIRE_START_TICK: u32 = 1_212;
const FIRE_HOLD_TICKS: u32 = 4;
const WEAPON_VIDEO_FIRE_FIRST_GAME_FRAME: u16 = 319;
const WEAPON_VIDEO_FIRE_LAST_GAME_FRAME: u16 = 321;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceDraw {
    pub list_order: usize,
    pub position: [i16; 3],
    pub rotation: [u8; 3],
    pub shape: u16,
    pub sort_depth: i16,
    pub strategy_flags: u8,
    pub color_table: u16,
    pub explosion_count: u8,
    pub animation: u8,
    pub color_frame: u8,
    pub depth_offset: u8,
    pub texture_scroll: [u8; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeProjection {
    pub list_order: usize,
    pub position: [i16; 3],
    pub shape: u16,
    pub object_light: [i8; 3],
    pub points: Vec<[i16; 2]>,
}

fn gsu_word(machine: &RetailMachine, address: usize) -> u16 {
    u16::from_le_bytes([
        machine.peek_gsu_ram(address),
        machine.peek_gsu_ram(address + 1),
    ])
}

fn retail_flat_shape(retail: &RetailMachine, source_word: u16) -> u16 {
    if let Some((_, native_shape)) = RETAIL_INTRO_SHAPE_IDS
        .iter()
        .find(|(retail_shape, _)| *retail_shape == source_word)
    {
        return *native_shape;
    }
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

fn retail_source_draw(machine: &RetailMachine, list_order: usize, base: usize) -> SourceDraw {
    SourceDraw {
        list_order,
        position: [
            gsu_word(machine, base + DRAW_POSITION_X) as i16,
            gsu_word(machine, base + DRAW_POSITION_Y) as i16,
            gsu_word(machine, base + DRAW_POSITION_Z) as i16,
        ],
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
}

/// Decode entries in the exact linked order consumed by `mshow`. The source
/// allocates entries densely, but its renderer starts at `m_dlptr` and follows
/// each entry's leading link word; storage order is not presentation order.
pub fn retail_source_draws(machine: &RetailMachine) -> Vec<SourceDraw> {
    let count = usize::from(gsu_word(machine, GSU_DRAW_COUNT));
    let list_end = GSU_DRAW_LIST + count * DRAW_ENTRY_BYTES;
    let mut address = usize::from(gsu_word(machine, GSU_DRAW_HEAD));
    let mut draws = Vec::with_capacity(count);
    let mut visited = std::collections::HashSet::with_capacity(count);
    while address != 0 {
        assert!(
            (GSU_DRAW_LIST..list_end).contains(&address),
            "retail draw link {address:#06X} outside allocated list"
        );
        assert_eq!(
            (address - GSU_DRAW_LIST) % DRAW_ENTRY_BYTES,
            0,
            "unaligned retail draw link"
        );
        assert!(visited.insert(address), "cyclic retail draw chain");
        let list_order = draws.len();
        draws.push(retail_source_draw(machine, list_order, address));
        address = usize::from(gsu_word(machine, address));
    }
    assert_eq!(draws.len(), count, "retail draw chain length");
    draws
}

fn native_source_draw_storage(shell: &Shell) -> Vec<SourceDraw> {
    let camera = shell.frame().camera;
    let camera_position = [
        (camera.x >> 16) as i16,
        (camera.y >> 16) as i16,
        (camera.z >> 16) as i16,
    ];
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
            let world = [
                (draw.x >> 16) as i16,
                (draw.y >> 16) as i16,
                (draw.z >> 16) as i16,
            ];
            let relative = [
                world[0].wrapping_sub(camera_position[0]),
                world[1].wrapping_sub(camera_position[1]),
                world[2].wrapping_sub(camera_position[2]),
            ];
            let position = sf_core::snes_trig::matrix_rotate_q15(
                matrix,
                relative[0],
                relative[1],
                relative[2],
            );
            let shape_sort_depth = sf_core::sf1_shape_metrics::sf1_shape_metrics(draw.shape_id)
                .map_or(0, |metrics| metrics.sort_depth);
            SourceDraw {
                list_order,
                position: [position.0, position.1, position.2],
                rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                shape: draw.shape_id,
                sort_depth: draw
                    .sort_z
                    .wrapping_add(position.2)
                    .wrapping_add(shape_sort_depth),
                strategy_flags: draw.sflags
                    | if draw.flags & sf_core::dl_flags::SCALED_SPRITE != 0 {
                        sf_game::alien::ASF_SSPRITE
                    } else {
                        0
                    },
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

/// Return the native entries in the same stable far-to-near order as the
/// source linked list and the strict source renderer.
pub fn native_source_draws(shell: &Shell) -> Vec<SourceDraw> {
    let mut draws = native_source_draw_storage(shell);
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

pub fn native_source_projections(shell: &Shell) -> Vec<NativeProjection> {
    let camera = shell.frame().camera;
    let source_draws = native_source_draw_storage(shell);
    shell
        .draw_list()
        .iter()
        .enumerate()
        .filter_map(|(list_order, draw)| {
            if draw.flags & sf_core::dl_flags::SCALED_SPRITE != 0 {
                return None;
            }
            let shape = sf_render::shapes::resolve_shape_word(draw.shape_id);
            let metrics = sf_core::sf1_shape_metrics::sf1_shape_metrics(shape)?;
            let shape_data = sf_render::shape_data::SHAPE_DATA
                .iter()
                .find(|entry| entry.shape_id == shape)?;
            let vertices = if shape_data.animation_frames.is_empty() {
                shape_data.vertices
            } else {
                shape_data.animation_frames
                    [usize::from(draw.anim_frame) % shape_data.animation_frames.len()]
            };
            let projected = sf_render::source_projection::project_shape(
                vertices,
                shape_data.reflected_pair_starts,
                metrics.coordinate_shift,
                sf_render::source_projection::SourcePose {
                    world_position: [
                        (draw.x >> 16) as i16,
                        (draw.y >> 16) as i16,
                        (draw.z >> 16) as i16,
                    ],
                    rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                    view_position: [
                        (camera.x >> 16) as i16,
                        (camera.y >> 16) as i16,
                        (camera.z >> 16) as i16,
                    ],
                    view_rotation: camera.rotation,
                },
            );
            Some(NativeProjection {
                list_order,
                position: source_draws[list_order].position,
                shape,
                object_light: projected.object_light,
                points: projected
                    .points
                    .into_iter()
                    .map(|point| [point.x, point.y])
                    .collect(),
            })
        })
        .collect()
}

pub fn native_source_shadow_projections(shell: &Shell) -> Vec<NativeProjection> {
    let frame = shell.frame();
    let camera = frame.camera;
    let camera_position = [
        (camera.x >> 16) as i16,
        (camera.y >> 16) as i16,
        (camera.z >> 16) as i16,
    ];
    let view_matrix = sf_core::snes_trig::zxy_matrix_q15_fine(
        camera.rotation[0],
        camera.rotation[1],
        camera.rotation[2],
    );
    shell
        .draw_list()
        .iter()
        .enumerate()
        .filter_map(|(list_order, draw)| {
            if draw.flags & sf_core::dl_flags::SHADOW == 0 {
                return None;
            }
            let shape = sf_render::shapes::resolve_shape_word(draw.shape_id);
            let metrics = sf_core::sf1_shape_metrics::sf1_shape_metrics(shape)?;
            let shape_data = sf_render::shape_data::SHAPE_DATA
                .iter()
                .find(|entry| entry.shape_id == shape)?;
            let vertices = if shape_data.animation_frames.is_empty() {
                shape_data.vertices
            } else {
                shape_data.animation_frames
                    [usize::from(draw.anim_frame) % shape_data.animation_frames.len()]
            };
            let world_position = [
                (draw.x >> 16) as i16,
                frame.scene_style.shadow_height,
                (draw.z >> 16) as i16,
            ];
            let relative = [
                world_position[0].wrapping_sub(camera_position[0]),
                world_position[1].wrapping_sub(camera_position[1]),
                world_position[2].wrapping_sub(camera_position[2]),
            ];
            let position = sf_core::snes_trig::matrix_rotate_q15(
                view_matrix,
                relative[0],
                relative[1],
                relative[2],
            );
            let projected = sf_render::source_projection::project_shadow_shape(
                vertices,
                shape_data.reflected_pair_starts,
                metrics.coordinate_shift,
                sf_render::source_projection::SourcePose {
                    world_position,
                    rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                    view_position: camera_position,
                    view_rotation: camera.rotation,
                },
            );
            Some(NativeProjection {
                list_order,
                position: [position.0, position.1, position.2],
                shape,
                object_light: projected.object_light,
                points: projected
                    .points
                    .into_iter()
                    .map(|point| [point.x, point.y])
                    .collect(),
            })
        })
        .collect()
}

pub fn native_source_exploded_shadow_faces(shell: &Shell) -> Vec<NativeProjection> {
    let frame = shell.frame();
    let camera = frame.camera;
    shell
        .draw_list()
        .iter()
        .filter(|draw| draw.flags & sf_core::dl_flags::SHADOW != 0 && draw.explosion_cnt != 0)
        .flat_map(|draw| {
            let shape = sf_render::shapes::resolve_shape_word(draw.shape_id);
            let metrics = sf_core::sf1_shape_metrics::sf1_shape_metrics(shape)
                .expect("exploding source shape metrics");
            let shape_data = sf_render::shape_data::SHAPE_DATA
                .iter()
                .find(|entry| entry.shape_id == shape)
                .expect("exploding source shape data");
            let vertices = if shape_data.animation_frames.is_empty() {
                shape_data.vertices
            } else {
                shape_data.animation_frames
                    [usize::from(draw.anim_frame) % shape_data.animation_frames.len()]
            };
            shape_data
                .faces
                .iter()
                .enumerate()
                .map(move |(face_index, face)| {
                    let projected = sf_render::source_projection::project_exploded_shadow_face(
                        vertices,
                        shape_data.reflected_pair_starts,
                        &face.vertex_indices[..usize::from(face.num_verts)],
                        face.normal,
                        metrics.coordinate_shift,
                        draw.explosion_cnt,
                        sf_render::source_projection::SourcePose {
                            world_position: [
                                (draw.x >> 16) as i16,
                                frame.scene_style.shadow_height,
                                (draw.z >> 16) as i16,
                            ],
                            rotation: [draw.rx as u8, draw.ry as u8, draw.rz as u8],
                            view_position: [
                                (camera.x >> 16) as i16,
                                (camera.y >> 16) as i16,
                                (camera.z >> 16) as i16,
                            ],
                            view_rotation: camera.rotation,
                        },
                    );
                    NativeProjection {
                        list_order: face_index,
                        position: projected.view_position,
                        shape,
                        object_light: projected.object_light,
                        points: projected
                            .points
                            .into_iter()
                            .map(|point| [point.x, point.y])
                            .collect(),
                    }
                })
        })
        .collect()
}

pub fn configured_shell() -> Shell {
    let mut shell = Shell::new();
    shell.set_register_strats(Box::new(sf_strat::table::register_all));
    shell.set_spawn_player(Box::new(sf_strat::player::strat_spawn_player));
    shell.set_advance_startup_player(Box::new(
        sf_strat::player::advance_player_during_level_initialization,
    ));
    shell.set_initialize_player(Box::new(sf_strat::player::initialize_player_for_map));
    shell.set_prepare_presentation_player(Box::new(sf_strat::player::prepare_presentation_player));
    shell.set_prepare_restart_player(Box::new(
        sf_strat::player::prepare_checkpoint_restart_player,
    ));
    shell.set_shape_extents(sf_render::shapes::sf1_shape_half_extents());
    shell
}

pub fn weapon_input(tick: u32) -> u16 {
    if (GAME_DESTINATION_SELECT_TICK..GAME_DESTINATION_SELECT_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::DOWN;
    }
    if (GAME_DESTINATION_CONFIRM_TICK..GAME_DESTINATION_CONFIRM_TICK + FRONT_END_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if tick <= FRONT_END_LAST_CONFIRM_TICK
        && tick % FRONT_END_CONFIRM_CADENCE_TICKS < FRONT_END_CONFIRM_HOLD_TICKS
    {
        return pad::START;
    }
    if (ROUTE_SELECTION_CONFIRM_TICK
        ..ROUTE_SELECTION_CONFIRM_TICK + ROUTE_SELECTION_CONFIRM_HOLD_TICKS)
        .contains(&tick)
    {
        return pad::START;
    }
    if (PLANET_DISMISS_START_TICK..PLANET_DISMISS_END_TICK).contains(&tick) {
        return if (tick - PLANET_DISMISS_START_TICK) % PLANET_DISMISS_CADENCE_TICKS == 0 {
            pad::B
        } else {
            0
        };
    }
    if (FIRE_START_TICK..FIRE_START_TICK + FIRE_HOLD_TICKS).contains(&tick) {
        return pad::Y;
    }
    0
}

/// Match the external Mesen source-video capture, which starts gameplay input
/// from the retail game-frame counter after the variable-rate front end.
pub fn weapon_video_input(shell: &Shell, tick: u32) -> u16 {
    let active = shell.state() == sf_game::shell::GameState::Playing
        && shell.frame().gameplay_entry_phase == sf_game::shell::GameplayEntryPhase::ActiveLevel;
    if !active {
        return weapon_input(tick);
    }
    let front_end_input = weapon_input(tick) & !pad::Y;
    front_end_input
        | if (WEAPON_VIDEO_FIRE_FIRST_GAME_FRAME..=WEAPON_VIDEO_FIRE_LAST_GAME_FRAME)
            .contains(&shell.game.vars.gameframe)
        {
            pad::Y
        } else {
            0
        }
}

pub fn render_entry(entry: &sf_core::DrawListEntry) -> RenderDrawListEntry {
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
        interpolation_id: entry.interpolation_id,
    }
}

/// Render the native SF1 frame at the fixed-update presentation boundary used
/// by the app. The returned list becomes the preceding presentation list for
/// the next call.
pub fn render_playing_frame(
    shell: &Shell,
    previous_draw_list: &[RenderDrawListEntry],
    renderer: &mut Renderer,
) -> (Vec<RenderDrawListEntry>, Vec<u8>) {
    let frame = shell.frame();
    let current_draw_list: Vec<_> = shell.draw_list().iter().map(render_entry).collect();
    let rgb = render_playing_snapshot(&frame, previous_draw_list, &current_draw_list, renderer);
    (current_draw_list, rgb)
}

pub fn render_playing_snapshot(
    frame: &FrameSnapshot,
    previous_draw_list: &[RenderDrawListEntry],
    current_draw_list: &[RenderDrawListEntry],
    renderer: &mut Renderer,
) -> Vec<u8> {
    let inputs = playing_frame_inputs(frame);
    let camera = frame.camera;
    renderer
        .transform
        .set_camera_fine(camera.x, camera.y, camera.z, camera.rotation);
    if camera.snap {
        renderer.transform.snap_camera();
    }
    renderer.begin_frame();
    // A completed source update presents the current draw list.  The app's
    // interpolation alpha starts at zero immediately after an update, where
    // `DrawListRenderer` intentionally selects its preceding list for smooth
    // real-time interpolation.  This strict fixed-boundary oracle has no
    // fractional presentation interval, so select the completed list exactly.
    renderer.submit(previous_draw_list, &current_draw_list, 1.0, &inputs);
    renderer.end_frame();
    renderer.read_pixels_rgb()
}

/// Compose the source bitmap completed by `scene` with the live presentation
/// state observed one gameplay update later. Retail transfers the Super FX
/// bitmap over intervening video frames, while BG2, CGRAM, windows, and final
/// display controls continue to advance. This is an oracle capture contract;
/// neither side's game state is modified or substituted.
pub fn presentation_aligned_source_frame(
    scene: &FrameSnapshot,
    presentation: &FrameSnapshot,
) -> FrameSnapshot {
    sf_game::presentation::compose_source_presentation(scene, presentation)
}

#[cfg(test)]
mod presentation_alignment_tests {
    use super::*;

    #[test]
    fn source_bitmap_uses_the_live_presentation_brightness() {
        const SCENE_BRIGHTNESS: u8 = 15;
        const PRESENTATION_BRIGHTNESS: u8 = 14;

        let scene = FrameSnapshot {
            display_brightness: SCENE_BRIGHTNESS,
            ..FrameSnapshot::default()
        };
        let presentation = FrameSnapshot {
            display_brightness: PRESENTATION_BRIGHTNESS,
            ..FrameSnapshot::default()
        };

        let aligned = presentation_aligned_source_frame(&scene, &presentation);
        assert_eq!(aligned.display_brightness, PRESENTATION_BRIGHTNESS);
    }

    #[test]
    fn source_bitmap_uses_the_live_presentation_blank_state() {
        let scene = FrameSnapshot {
            display_forced_blank: true,
            ..FrameSnapshot::default()
        };
        let presentation = FrameSnapshot {
            display_forced_blank: false,
            ..FrameSnapshot::default()
        };

        let aligned = presentation_aligned_source_frame(&scene, &presentation);
        assert!(!aligned.display_forced_blank);
    }
}

pub fn render_presentation_aligned_source_frame(
    scene: &FrameSnapshot,
    presentation: &FrameSnapshot,
    draw_list: &[RenderDrawListEntry],
    renderer: &mut Renderer,
) -> Vec<u8> {
    let aligned = presentation_aligned_source_frame(scene, presentation);
    let mut inputs = playing_frame_inputs(&aligned);
    inputs.source_background_pitch = Some(scene.camera.rotation[0]);
    if std::env::var_os("SF1_WEAPON_DISABLE_TYPED_HORIZONTAL_OFFSETS").is_some() {
        inputs.bg2_horizontal_offsets = None;
    }
    if std::env::var_os("SF1_TRAINING_DISABLE_TYPED_VERTICAL_OFFSETS").is_some() {
        inputs.bg2_vertical_offsets = None;
    }
    if let Some(delta) = std::env::var("SF1_TRAINING_DIAGNOSTIC_VERTICAL_OFFSET_DELTA")
        .ok()
        .map(|value| value.parse::<i16>().expect("signed vertical offset delta"))
    {
        inputs.bg2_vertical_offsets = inputs
            .bg2_vertical_offsets
            .map(|offsets| offsets.map(|offset| offset.wrapping_add(delta)));
    }
    inputs.source_scene_camera = Some(SourceSceneCamera {
        position: [scene.camera.x, scene.camera.y, scene.camera.z],
        rotation: scene.camera.rotation,
    });
    if std::env::var_os("SF1_TRAINING_ISOLATE_BACKGROUND").is_some() {
        inputs.game_state = RenderGameState::Playing;
        inputs.stayblack = 0;
        inputs.stage_banner = None;
        inputs.display_forced_blank = false;
        inputs.display_black_subtraction = 0;
        inputs.screen_wipe = sf_core::screen_wipe::ScreenWipeState::inactive();
    }
    let camera = presentation.camera;
    renderer
        .transform
        .set_camera_fine(camera.x, camera.y, camera.z, camera.rotation);
    if camera.snap {
        renderer.transform.snap_camera();
    }
    renderer.begin_frame();
    renderer.submit(&[], draw_list, 1.0, &inputs);
    renderer.end_frame();
    renderer.read_pixels_rgb()
}

pub fn playing_frame_inputs(frame: &FrameSnapshot) -> FrameInputs<'_> {
    frame_inputs(frame, RenderGameState::Playing)
}

pub fn frame_inputs(frame: &FrameSnapshot, game_state: RenderGameState) -> FrameInputs<'_> {
    let mut windows = [WindowState::default(); WINDOWARRAY_SIZE];
    for (destination, source) in windows.iter_mut().zip(frame.windows) {
        *destination = WindowState {
            mode: source.mode,
            wm_val: source.wm_val,
            stayblack: source.stayblack,
        };
    }
    FrameInputs {
        source_resolution: true,
        source_background_pitch: Some(frame.camera.rotation[0]),
        game_state,
        currentbg: frame.currentbg,
        newmap: frame.newmap,
        bgflags: frame.bgflags,
        bg2_xscroll: frame.bg2_xscroll,
        bg2_vertical_offsets: frame.bg2_vertical_offsets,
        bg2_horizontal_offsets: frame.bg2_horizontal_offsets,
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
        meters: frame.meters,
        stayblack: frame.stayblack,
        gameflags: frame.gameflags,
        gameframe: frame.gameframe,
        briefing_phase: frame.briefing_phase,
        briefing_choice: frame.briefing_choice,
        control_type: frame.control_type,
        boostcnt: frame.boostcnt,
        arrows: frame.arrows,
        player_view_mode: frame.player_view_mode,
        stage: frame.stage,
        stage_banner: frame.stage_banner,
        scramble_banner: frame.scramble_banner,
        shield_cur: frame.shield_cur,
        shield_max: frame.shield_max,
        boss_hp_cur: frame.boss_hp_cur,
        boss_hp_max: frame.boss_hp_max,
        lives: frame.lives,
        bombs: frame.bombs,
        specflash: frame.specflash,
        shieldup: frame.shieldup,
        msg_count1: frame.radio_presentation.count,
        msg_count2: frame.radio_presentation.opening_frame,
        radio_face_frame: frame.radio_presentation.portrait_frame,
        whichfriend: frame.radio_presentation.speaker,
        friends_meter: frame.radio_presentation.teammate_meter,
        message_text: frame.radio_presentation.text.as_deref(),
        whichroute: frame.whichroute,
        currentplanet: frame.currentplanet,
        nebula_on: frame.nebula_on,
        route_path_ids: &frame.route_path_ids,
        planet_presentation: frame.planet_presentation,
        score: frame.score_total,
        credits: frame.credits,
        tally_active: frame.tally_active,
        tally_stage_perc: frame.tally_stage_perc,
        tally_current_perc: frame.tally_current_perc,
        tally_teammate_shields: frame.tally_teammate_shields,
        tally_bonus_visible: frame.tally_bonus_visible,
        ..FrameInputs::default()
    }
}
