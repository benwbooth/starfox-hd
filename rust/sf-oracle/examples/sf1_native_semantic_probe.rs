//! Emit typed native Corneria state for independent Mesen comparison.

#[path = "support/mod.rs"]
mod support;

use sf_game::shell::{GameState, GameplayEntryPhase};
use sf_oracle::sf1_input::{corneria_attack_carrier_input, corneria_front_end_input};
use std::collections::BTreeSet;

const DEFAULT_EARLY_SCENES: [u16; 6] = [1, 187, 307, 607, 807, 907];
const RESTART_FIRST_SCENE: u16 = 940;
const RESTART_LAST_SCENE: u16 = 983;
const REPLAY_TICK_BUDGET: u32 = 4_000;
const PLAYER_SLOT: usize = 0;
const PLAYER_BODY_SLOT: usize = 1;

fn requested_scenes() -> BTreeSet<u16> {
    if let Ok(range) = std::env::var("SF1_NATIVE_SEMANTIC_RANGE") {
        let (first, last) = range
            .split_once('-')
            .expect("native semantic range must be FIRST-LAST");
        let first: u16 = first
            .parse()
            .expect("native semantic first scene is decimal");
        let last: u16 = last.parse().expect("native semantic last scene is decimal");
        assert!(first <= last, "native semantic range must be ordered");
        return (first..=last).collect();
    }
    if let Ok(scenes) = std::env::var("SF1_NATIVE_SEMANTIC_SCENES") {
        return scenes
            .split(',')
            .map(|value| {
                value
                    .parse()
                    .expect("native semantic scene must be decimal")
            })
            .collect();
    }
    DEFAULT_EARLY_SCENES
        .into_iter()
        .chain(RESTART_FIRST_SCENE..=RESTART_LAST_SCENE)
        .collect()
}

fn slot_order(head: Option<u16>, objects: &sf_game::obj::Objects) -> Vec<u16> {
    let mut slots = Vec::new();
    let mut current = head;
    while let Some(slot) = current {
        slots.push(slot);
        assert!(
            slots.len() <= sf_game::alien::NUMBER_AL,
            "native object list contains a cycle"
        );
        current = objects.aliens[usize::from(slot)].next;
    }
    slots
}

fn joined(slots: &[u16]) -> String {
    slots
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn emit_scene(shell: &sf_game::shell::Shell, include_objects: bool) {
    let vars = &shell.game.vars;
    let objects = &shell.game.objs;
    let active_order = objects.active_indices();
    let free_order = slot_order(objects.free_head, objects);
    let camera = shell.frame().camera;
    let player = objects.aliens[PLAYER_SLOT];
    let player_body = objects.aliens[PLAYER_BODY_SLOT];
    println!(
        "kind=semantic scene={} background={} game_flags={} player_ship_flags={} player_ship_flags_2={} player_ship_flags_3={} player_strategy_flags={} player_fly_mode={} player_object={} map_countdown={} view_kind={} player_view_x={} player_view_y={} player_view_z={} view_float_x={} view_float_y={} view_shake_x={} view_shake_y={} view_shake_z={} view_position_x={} view_position_y={} view_position_z={} view_pitch={} view_yaw={} effective_view_yaw={} view_distance={} forward_velocity={} previous_player_depth={} last_depth_change={} player_hit_timer={} player_hit_flags={} player_body_durability={} presentation_rotation={} presentation_vertical={} presentation_boost_delay={} message_count={} message_opening_frame={} message_speaker={} random_0={} random_1={} random_2={} random_3={} active_order={} free_order={}",
        vars.gameframe,
        vars.currentbg,
        vars.gameflags,
        vars.pshipflags,
        vars.pshipflags2,
        vars.pshipflags3,
        vars.pstratflags,
        vars.playerflymode,
        vars.player_object,
        vars.mapcnt,
        vars.strategy.view_kind,
        vars.strategy.player_view_position[0],
        vars.strategy.player_view_position[1],
        vars.strategy.player_view_position[2],
        vars.strategy.view_float_x,
        vars.strategy.view_float_y,
        vars.strategy.view_shake[0] as i8,
        vars.strategy.view_shake[1] as i8,
        vars.strategy.view_shake[2] as i8,
        (camera.x >> 16) as i16,
        (camera.y >> 16) as i16,
        (camera.z >> 16) as i16,
        vars.strategy.view_pitch,
        vars.strategy.view_yaw,
        camera.rotation[1] as i16,
        vars.strategy.view_distance,
        vars.pviewvelz,
        shell.game.world.lastplayz,
        shell.game.world.lastzchange,
        player.sbyte1,
        player.hitflags,
        player_body.hp,
        vars.strategy.player_bytes[0],
        vars.strategy.player_bytes[1],
        vars.strategy.player_bytes[2],
        shell.frame().msg_count1,
        shell.frame().msg_count2,
        shell.frame().whichfriend,
        vars.rng[0],
        vars.rng[1],
        vars.rng[2],
        vars.rng[3],
        joined(&active_order),
        joined(&free_order),
    );
    if !include_objects {
        return;
    }
    for slot in active_order {
        let object = objects.aliens[usize::from(slot)];
        let source_flags = u16::from(object.flags) | (u16::from(object.type_) << 8);
        println!(
            "kind=semantic_object scene={} slot={} shape={} flags={} x={} y={} z={} durability={} hit_flags={} collision_flags={} damage_flags={} hit_timer={} path_wait={} rotation_x={} rotation_y={} rotation_z={} speed={} velocity_x={} velocity_y={} velocity_z={} vertical_offset={}",
            vars.gameframe,
            slot,
            object.shape,
            source_flags,
            object.worldx,
            object.worldy,
            object.worldz,
            object.hp,
            object.hitflags,
            object.sflags2,
            object.sflags3,
            object.sbyte1,
            object.sbyte3,
            object.rotx,
            object.roty,
            object.rotz,
            object.vel,
            object.vx,
            object.vy,
            object.vz,
            object.sword2,
        );
    }
}

fn main() {
    let requested = requested_scenes();
    assert!(
        !requested.is_empty(),
        "at least one semantic scene is required"
    );
    let final_scene = *requested.last().expect("requested semantic scene");
    let routed = std::env::var_os("SF1_NATIVE_SEMANTIC_ROUTE").is_some();
    let include_objects = std::env::var_os("SF1_NATIVE_SEMANTIC_NO_OBJECTS").is_none();
    let mut shell = support::configured_shell();
    let mut emitted = BTreeSet::new();

    for tick in 0..REPLAY_TICK_BUDGET {
        let active = shell.state() == GameState::Playing
            && shell.frame().gameplay_entry_phase == GameplayEntryPhase::ActiveLevel;
        let input = if active && routed {
            corneria_attack_carrier_input(shell.game.vars.gameframe)
        } else if active {
            0
        } else {
            corneria_front_end_input(tick)
        };
        shell.tick(input);
        if shell.frame().gameplay_entry_phase != GameplayEntryPhase::ActiveLevel {
            continue;
        }
        let scene = shell.game.vars.gameframe;
        if requested.contains(&scene) && emitted.insert(scene) {
            emit_scene(&shell, include_objects);
        }
        if scene >= final_scene {
            break;
        }
    }

    assert_eq!(emitted, requested, "native replay missed semantic scenes");
}
