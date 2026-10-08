//! Attract-loop scene start: the stage hand-over's scene-visible stores and
//! the attract map records that select the scene (`$05:FBE7`, `$05:8035`).
//! Only stores consumed by the scene services are modeled here; presentation
//! (scene loads, display mode, music) belongs to its own owners.

use super::path_program::ActionGate;
use super::scene_map::{self, MapActorSpawn};
use super::scene_path_world::ScenePathWorld;
use super::path_scene_state::EncounterHandoff;
use super::view_transition::{FixedViewAngles, ViewTransitionMode};
use super::{Behavior, Object, ObjectId, ObjectKind, ObjectSpawnDefaults, ObjectStore, ShapeId, Vector3};

/// Shared mode word (1B84): `$03:83C1` sets bit 0010, `$03:83C7` clears 0100.
const STAGE_MODE_SET: u16 = 0x0010;
const STAGE_MODE_CLEAR: u16 = 0x0100;
/// `$03:8316`: the scene frame loop's mode bit, set once at boot.
const FRAME_LOOP_MODE: u16 = 0x0080;
/// Spawn group published by the shared reset (no selected region).
const NO_SELECTED_REGION: u8 = u8::MAX;
/// `$03:8A68`: the scene frame loop reseeds the generator before its first
/// epoch, after the map has selected the scene.
const SCENE_LOOP_SEED: [u8; 4] = [0x3A, 0xA7, 0x55, 0x7F];
/// Map records `5C 01 721D00` / `5C 00 741D00`.
const SCENE_GATE: u8 = 1;
const SCENE_HANDOFF_FLAGS: u8 = 0;

/// Map record `$05:8003` (opcode 86): the scene player, strategy `$06:82F9`.
const SCENE_PLAYER_SPAWN: MapActorSpawn = MapActorSpawn {
    kind: ObjectKind::Player,
    shape: ShapeId::EMPTY,
    behavior: Behavior::PlayerSceneInit,
    position: Vector3 { x: 400, y: -150, z: 0 },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttractStageError {
    MissingViewMode,
    MissingHandoff,
}

/// Stage hand-over stores, in source order: `$03:B84F` clears 1AA6 and
/// `$03:B246` publishes its single-player byte (02); `$03:C275` clears the
/// cinematic word 1B96; `$03:83C1/83C7` adjust the mode word.
pub fn hand_over(world: &mut ScenePathWorld) -> Result<(), AttractStageError> {
    world.reflect_all_contacts = Some(true);
    world.cinematic_signals = Some(Default::default());
    world.reticle_inhibited = Some(false);
    let mode = world
        .view_transition_mode
        .as_mut()
        .ok_or(AttractStageError::MissingViewMode)?;
    mode.flags = (mode.flags | STAGE_MODE_SET) & !STAGE_MODE_CLEAR;
    Ok(())
}

/// The scene-selecting map records after the scene player is initialized.
pub fn select_scene(world: &mut ScenePathWorld, selection: u8) -> Result<(), AttractStageError> {
    world
        .handoff
        .as_mut()
        .ok_or(AttractStageError::MissingHandoff)?
        .player_flags = SCENE_HANDOFF_FLAGS;
    world.action_gate = Some(ActionGate { code: SCENE_GATE });
    world.scene_selection = Some(selection);
    Ok(())
}

/// The scene frame loop's start (`$03:8A62`), before its first epoch.
pub fn start_frame_loop(world: &mut ScenePathWorld) {
    world.random.reseed(SCENE_LOOP_SEED);
}

/// The scene-player spawn record. Its inline continuation (`$05:8011`)
/// spawns a second player only without the single-player policy (1AA6 bit
/// 02), which the attract loop always publishes.
pub fn spawn_scene_player(
    objects: &mut ObjectStore,
    defaults: ObjectSpawnDefaults,
) -> Option<ObjectId> {
    scene_map::allocate_map_actor(objects, defaults, SCENE_PLAYER_SPAWN)
}

/// The scene-six stage start (`$03:BF71..BF91`) returns the fixed view to
/// the origin with zero angles, including their fractions. Scene seven's
/// stage keeps the previous scene's view.
pub fn reset_view(view: &mut Object) {
    view.base.position = Vector3::default();
    FixedViewAngles { pitch: 0, yaw: 0, roll: 0 }.write_to(view);
}

/// The world as boot leaves it for the first attract scene: the scene
/// frame loop's mode bit and the attract pilot's (Fox's) shield loadout
/// (`$06:A3D9`, table `$06:A46E`); every other scene publication is clear.
/// The scene palette is clear until the first scene's loader uploads it.
pub fn boot_world() -> ScenePathWorld {
    let pilot = super::Pilot::Fox;
    let shield = pilot.craft_profile().maximum_shield;
    let mut world = ScenePathWorld::new(super::RandomState::default());
    let mode = ViewTransitionMode { flags: FRAME_LOOP_MODE };
    world.view_transition_mode = Some(mode);
    world.spawn_defaults = Some(mode.spawn_defaults(ObjectSpawnDefaults {
        group: NO_SELECTED_REGION,
        run_when_paused: false,
    }));
    world.published_score = Some(Default::default());
    world.active_shield_capacity = Some(shield);
    world.scene.active_shield = Some(shield);
    world.scene.active_pilot = Some(0);
    world.scene.wingmate_pilot = Some(0);
    world.scene.player_configuration = Some(0);
    world.scene.encounter_location = Some(0);
    world.handoff = Some(EncounterHandoff { player_flags: 0, x: 0, z: 0, heading_word: 0 });
    world.campaign_phase = Some(0);
    world.cinematic_signals = Some(Default::default());
    world.reticle_inhibited = Some(false);
    world.reflect_all_contacts = Some(true);
    world.palette = Some(super::player_action::ScenePalette {
        colors: [0; super::player_action::SCENE_PALETTE_COLORS],
        saved_colors: [0; super::player_action::SCENE_PALETTE_COLORS],
    });
    world.palette_refresh_requested = Some(false);
    world.player_service_flags = Some(Default::default());
    world.controller_inputs = [Some(Default::default()); 2];
    world.encounter_signals = Some(Default::default());
    world.camera_height_limits = Some((0, 0));
    world.camera_projection_offset = Some(0);
    world.published_camera_projection = Some(0);
    world.weapons = Some(Default::default());
    world.published_motion = Some(Default::default());
    world.engine_sound_control = Some(super::player_engine_sound::EngineSoundControl::from_bits(0));
    world.linked_effect_activity = Some(Default::default());
    world.camera_tracking = Some(Default::default());
    world
}

/// The excluded second-player slot (14D6): the source's cue-marker proxy,
/// never visited by the strategy pass. Its pose is the last proxy pose,
/// which the native markers do not use; it starts at the origin.
pub fn excluded_proxy(pose: Option<Vector3>) -> Object {
    let mut proxy = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
    proxy.base.position = pose.unwrap_or_default();
    proxy.base.hit_points = 0;
    proxy.base.contacts.first_strategy_visit = true;
    proxy
}
