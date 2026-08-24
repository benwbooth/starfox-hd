//! Strict retail/native trace for the first Corneria player-laser burst.
//!
//! Source addresses and cartridge storage are confined to this oracle adapter;
//! the native side is read only through typed flat game objects.

use sf_core::pad;
use sf_game::shell::{GameState, GameplayEntryPhase, Shell, SoundCmd};
use sf_oracle::{
    load_retail_rom, RetailMachine, AL_AP, AL_COLLFLAGS, AL_HP, AL_IMMUNEPTR, AL_LIFECNT, AL_ROTX,
    AL_ROTY, AL_ROTZ, AL_SBYTE1, AL_SBYTE2, AL_SBYTE3, AL_TYPE, AL_VEL, AL_VX, AL_VY, AL_VZ,
    RETAIL_AL_ANIMFRAME, RETAIL_BUILD_DRAWLIST_L, RETAIL_DOSTRATS, RETAIL_FRAMERATE,
    RETAIL_GAMEFRAME, RETAIL_PLAYPT, RETAIL_POOL, RETAIL_SOUND_EFFECT_EVENTS,
    RETAIL_SOUND_EFFECT_WRITE_CURSOR, RETAIL_VIEW_POSITION_X, RETAIL_VIEW_POSITION_Y,
    RETAIL_VIEW_POSITION_Z,
};

const WORK_RAM: u32 = 0x7E_0000;
const VIDEO_FRAMES_PER_NATIVE_TICK: u32 = 3;
const MAX_VIDEO_FRAMES_PER_LEVEL_UPDATE: u32 = 12;
const MAX_VIDEO_FRAMES_DURING_AUDIO_UPLOAD: u32 = 240;
const COMPLETED_FRAME_ALIGNMENT_TICK: u32 = 900;
const CORNERIA_AUDIO_UPLOAD_TICK: u32 = 1_080;
const FIRST_LEVEL_STATE_TICK: u32 = 892;
const FIRE_START_TICK: u32 = 1_212;
const TRACE_END_TICK: u32 = 1_230;
const FIRE_HOLD_TICKS: u32 = 4;
const PLAYER_LASER_SOUND: u8 = 53;
const SOUND_EVENT_CAPACITY: u8 = 16;
const SOURCE_PLAYER_LASER_SHAPE: u16 = 0xB369;
const NATIVE_PLAYER_LASER_SHAPE: u16 = 511;

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

const GSU_DRAW_COUNT: usize = 0x01B6;
const GSU_DRAW_LIST: usize = 0x0EF2;
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

fn retail_object_base(slot: u16) -> u32 {
    RETAIL_POOL.base + u32::from(slot) * RETAIL_POOL.stride
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
    let mut native = configured_shell();
    let mut retail_level_boundary_aligned = false;
    let mut previous_retail_level_frame = None;
    let mut certified_weapon_updates = 0;
    let mut first_weapon_tick = None;
    let mut native_laser_sound_count = 0;
    let mut retail_laser_sound_count = 0;
    let mut first_draw_divergence = None;

    for tick in 0..=TRACE_END_TICK {
        let input = scripted_input(tick);
        let next_input = scripted_input(tick.saturating_add(1));
        let native_level_active = native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let align_completed_level_frame =
            native_level_active && tick >= COMPLETED_FRAME_ALIGNMENT_TICK;
        let mut native_frame_rate_for_update = None;
        let mut retail_draws_for_update = None;
        let mut retail_view_for_update = None;
        let mut retail_sound_events_for_update = None;
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
            native_frame_rate_for_update = Some(retail.peek8(WORK_RAM | RETAIL_FRAMERATE));
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
            assert!(
                retail
                    .tick_until_cpu_execution(next_input, RETAIL_DOSTRATS, max_video_frames,)
                    .expect("next gameplay boundary"),
                "retail did not reach the next gameplay update {tick}"
            );
        } else {
            retail
                .tick_video_frames(input, VIDEO_FRAMES_PER_NATIVE_TICK)
                .expect("retail front-end update");
        }
        let retail_level_frame = retail.peek16(WORK_RAM | RETAIL_GAMEFRAME);
        let retail_completed_level_update = align_completed_level_frame
            || previous_retail_level_frame
                .map(|previous| previous != retail_level_frame)
                .unwrap_or(true);
        if !native_level_active || retail_completed_level_update {
            if let Some(frame_rate) = native_frame_rate_for_update {
                native.game.vars.strategy.frame_rate = frame_rate;
            }
            native.tick(input);
        }
        if native.state() == GameState::Playing
            && native.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel
        {
            previous_retail_level_frame = Some(retail_level_frame);
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
    println!(
        "sf1_weapon certified_updates={certified_weapon_updates} first_divergence=none first_weapon_tick={first_weapon_tick} sound_events={native_laser_sound_count} source_coverage=playerfire,fire_elaser,pelaser"
    );
}
