use super::*;
use crate::collision_surface::SurfaceMode;
use crate::path_invocation::InvocationWorld;
use crate::path_motion::PublishedPlayerMotion;
use crate::path_program::{ActionGate, PathCatalog};
use crate::path_scene_state::{CameraTrackingTarget, EncounterCameraFocus, EncounterHandoff};
use crate::path_target::{PublishedHomingTarget, TargetingUpgradeState};
use crate::player_action::PlayerServiceFlags;
use crate::player_hit_control::ShieldRecoveryRequest;
use crate::player_storage::{self, PlayerScore, PlayerStorageInputs};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, Buttons, Object, ObjectKind, RandomState, ShapeId, SoundEvent, Vector3};

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(RandomState::new([1, 7, 29, 255]));
    world.controller_inputs = [
        Some(InputState {
            held: Buttons::from_bits(0x1234),
            pressed: Buttons::from_bits(0x5678),
        }),
        Some(InputState {
            held: Buttons::from_bits(0x9876),
            pressed: Buttons::from_bits(0xABCD),
        }),
    ];
    world.processed_player_input = world.controller_inputs[1];
    world.unmasked_player_input = world.controller_inputs[1];
    world.scene.player_configuration = Some(9);
    world.interception_active = Some(true);
    world.interception_music_ready = Some(true);
    world.linked_effect_activity = Some(crate::path_protection::LinkedEffectActivity { recent_spawn: 173 });
    world.scene.active_shield = Some(42);
    world.active_shield_capacity = Some(32);
    world.weapons = Some(WeaponState {
        published_pitch: Some(Angle::from_units(113)),
        fallback: Some(owner),
        ..Default::default()
    });
    world.handoff = Some(EncounterHandoff {
        player_flags: 197,
        x: -321,
        z: 4321,
        heading_word: 0xABCD,
    });
    world.surface_mode = Some(SurfaceMode { flags: 255 });
    world.node_exit = crate::player_node_exit::NodeExitState {
        presentation_flags: Some(0xC7),
        completion_code: Some(255),
    };
    world.action_gate = Some(ActionGate { code: 231 });
    world.shield_recovery = Some(ShieldRecoveryRequest { amount: 76 });
    world.player_service_flags = Some(PlayerServiceFlags::from_bits(255));
    world.reticle_enabled = Some(true);
    world.target_reticle.horizontal = Some(183);
    world.target_reticle.vertical = Some(247);
    world.camera_tracking = Some(CameraTrackingTarget { actor: Some(owner) });
    let position = Vector3 {
        x: 193,
        y: -123,
        z: i16::MIN,
    };
    world.camera_focus = Some(EncounterCameraFocus { position });
    world.published_motion = Some(PublishedPlayerMotion {
        position,
        delta: position,
    });
    world.environment_plane_height = Some(1789);
    world.player_surface_support = Some(crate::player_motion::PlayerSurfaceSupport { object: Some(owner), group: 197 });
    (objects, world, owner)
}

#[test]
fn reset_publishes_only_owned_fields_and_keeps_other_owners_and_reticle_axis() {
    let (objects, mut world, owner) = fixture();
    let actor = objects.get(owner).unwrap().clone();
    let random = world.random;
    world.strategy_clock = 157;
    world.primary_player = Some(owner);
    world.secondary_player = Some(owner);
    world.fixed_players = [Some(owner); 2];
    world.scene.active_pilot = Some(5);
    world.scene.active_weapon_level = Some(4);
    world.scene.player_view_control = Some(0xA5);
    world.active_charge_threshold = Some(35);
    world.reticle_inhibited = Some(true);
    world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: 0xB6 });
    world.published_homing_target = Some(PublishedHomingTarget {
        object: Some(owner),
    });
    world.audio.queue(SoundEvent::RapidLaser);
    let mut expected_weapons = world.weapons.unwrap();
    expected_weapons.published_pitch = Some(Angle::ZERO);
    let mut expected_handoff = world.handoff.unwrap();
    expected_handoff.player_flags = 0;
    let sampled = world.controller_inputs;
    reset_services(&objects, &mut world, owner).unwrap();
    assert_eq!(objects.get(owner), Some(&actor));
    assert_eq!(world.random, random);
    assert_eq!(world.strategy_clock, 157);
    assert_eq!(world.primary_player, Some(owner));
    assert_eq!(world.secondary_player, Some(owner));
    assert_eq!(world.fixed_players, [Some(owner); 2]);
    assert_eq!(world.controller_inputs, sampled);
    assert_eq!(world.unmasked_player_input, sampled[1]);
    assert_eq!(world.processed_player_input, Some(InputState::default()));
    assert_eq!(world.scene.player_configuration, Some(0));
    assert_eq!(world.interception_active, Some(true));
    assert_eq!(world.interception_music_ready, Some(false));
    assert_eq!(world.linked_effect_activity, Some(Default::default()));
    assert_eq!(world.scene.active_shield, Some(32));
    assert_eq!(world.scene.active_pilot, Some(5));
    assert_eq!(world.scene.active_weapon_level, Some(4));
    assert_eq!(world.scene.player_view_control, Some(0xA5));
    assert_eq!(world.active_charge_threshold, Some(35));
    assert_eq!(world.weapons, Some(expected_weapons));
    assert_eq!(world.handoff, Some(expected_handoff));
    assert_eq!(world.surface_mode, Some(SurfaceMode::default()));
    assert_eq!(world.node_exit, crate::player_node_exit::NodeExitState {
        presentation_flags: Some(0),
        completion_code: Some(0),
    });
    assert_eq!(world.action_gate, Some(ActionGate::default()));
    assert_eq!(
        world.shield_recovery,
        Some(ShieldRecoveryRequest::default())
    );
    assert_eq!(
        world.player_service_flags,
        Some(PlayerServiceFlags::default())
    );
    assert_eq!(world.reticle_enabled, Some(false));
    assert_eq!(world.reticle_inhibited, Some(true));
    assert_eq!(world.target_reticle.horizontal, Some(100));
    assert_eq!(world.target_reticle.vertical, Some(247));
    assert_eq!(world.camera_tracking, Some(CameraTrackingTarget::default()));
    assert_eq!(world.camera_focus, Some(EncounterCameraFocus::default()));
    assert_eq!(
        world.published_motion,
        Some(PublishedPlayerMotion::default())
    );
    assert_eq!(world.environment_plane_height, Some(0));
    assert_eq!(world.player_surface_support, Some(Default::default()));
    assert_eq!(world.targeting_upgrade.unwrap().pilot_flags, 0xB6);
    assert_eq!(world.published_homing_target.unwrap().object, Some(owner));
    assert_eq!(world.audio.take_events()[0], Some(SoundEvent::RapidLaser));
    let inputs = world
        .path_world(&objects, owner, crate::path_control::PlayerTarget::Primary)
        .unwrap();
    assert_eq!(inputs.scene.active_shield, Some(32));
    assert_eq!(
        inputs.published_motion,
        Some(PublishedPlayerMotion::default())
    );
    assert_eq!(inputs.environment_plane_height, Some(0));
    assert_eq!(inputs.handoff.unwrap().player_flags, 0);
}

#[test]
fn shield_clamp_uses_every_wrapped_byte_difference() {
    let (objects, mut world, owner) = fixture();
    for capacity in 0..=u8::MAX {
        for shield in 0..=u8::MAX {
            world.active_shield_capacity = Some(capacity);
            world.scene.active_shield = Some(shield);
            reset_services(&objects, &mut world, owner).unwrap();
            let expected = if capacity.wrapping_sub(shield) & 0x80 != 0 {
                capacity
            } else {
                shield
            };
            assert_eq!(world.scene.active_shield, Some(expected));
        }
    }
}

#[test]
fn reset_samples_the_visiting_actor_side_and_never_requires_the_other_controller() {
    for (side, index) in [(HitSide::Primary, 0), (HitSide::Secondary, 1)] {
        let (mut objects, mut world, owner) = fixture();
        objects.get_mut(owner).unwrap().base.contacts.hit_side = side;
        world.primary_player = None;
        world.controller_inputs[1 - index] = None;
        reset_services(&objects, &mut world, owner).unwrap();
        world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(0xA55A),
            pressed: Buttons::from_bits(0x5AA5),
        });
        let previous = world.processed_player_input;
        world.controller_inputs[index] = None;
        world.scene.player_configuration = Some(9);
        assert_eq!(
            reset_services(&objects, &mut world, owner),
            Err(PlayerSceneResetError::MissingController(side))
        );
        assert_eq!(world.processed_player_input, previous);
        assert_eq!(world.scene.player_configuration, Some(9));
    }
}

#[test]
fn missing_actor_is_rejected_before_any_shared_publication() {
    let (mut objects, mut world, owner) = fixture();
    objects.remove(owner).unwrap();
    let processed = world.processed_player_input;
    assert_eq!(
        reset_services(&objects, &mut world, owner),
        Err(PlayerSceneResetError::World(WorldInputError::MissingActor(
            owner
        )))
    );
    assert_eq!(world.processed_player_input, processed);
    assert_eq!(world.scene.player_configuration, Some(9));
    assert_eq!(world.scene.active_shield, Some(42));
    assert_eq!(world.target_reticle.horizontal, Some(183));
}

#[test]
fn missing_owners_keep_the_exact_completed_service_prefix() {
    for stage in 0..4 {
        let (objects, mut world, owner) = fixture();
        let expected = match stage {
            0 => {
                world.active_shield_capacity = None;
                PlayerSceneResetError::MissingShieldCapacity
            }
            1 => {
                world.scene.active_shield = None;
                PlayerSceneResetError::MissingPublishedShield
            }
            2 => {
                world.weapons = None;
                PlayerSceneResetError::MissingWeaponState
            }
            _ => {
                world.handoff = None;
                PlayerSceneResetError::MissingHandoff
            }
        };
        assert_eq!(reset_services(&objects, &mut world, owner), Err(expected));
        assert_eq!(world.processed_player_input, Some(InputState::default()));
        assert_eq!(world.scene.player_configuration, Some(0));
        assert_eq!(world.interception_music_ready, Some(false));
        assert_eq!(world.linked_effect_activity, Some(Default::default()));
        assert_eq!(world.interception_active, Some(true));
        assert_eq!(
            world.scene.active_shield,
            match stage {
                0 => Some(42),
                1 => None,
                _ => Some(32),
            }
        );
        assert_eq!(
            world.surface_mode.unwrap().flags,
            if stage == 3 { 0 } else { 255 }
        );
        assert_eq!(world.reticle_enabled, Some(stage != 3));
        assert_eq!(world.node_exit.presentation_flags, Some(if stage == 3 { 0 } else { 0xC7 }));
        assert_eq!(world.node_exit.completion_code, Some(255));
        assert_eq!(
            world.action_gate.unwrap().code,
            if stage == 3 { 0 } else { 231 }
        );
        assert_eq!(world.shield_recovery.unwrap().amount, 76);
        assert_eq!(world.target_reticle.horizontal, Some(183));
        assert_eq!(world.target_reticle.vertical, Some(247));
        assert_eq!(world.environment_plane_height, Some(1789));
    }
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("no mode dispatch in a shared reset")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("no death dispatch")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("no map continuation")
    }
}

#[test]
fn scene_latches_a_partial_reset_instead_of_repeating_completed_writes() {
    let (mut objects, mut world, owner) = fixture();
    world.handoff = None;
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut Callbacks,
        statement_budget: 1,
    };
    assert_eq!(
        host.reset_player_services(owner),
        Err(SceneError::PlayerSceneReset(
            PlayerSceneResetError::MissingHandoff
        ))
    );
    assert!(host.execution.is_faulted());
    host.world.handoff = Some(EncounterHandoff::default());
    host.world.scene.active_shield = Some(91);
    assert_eq!(host.reset_player_services(owner), Err(SceneError::Faulted));
    assert_eq!(host.world.scene.active_shield, Some(91));
    assert_eq!(host.world.target_reticle.horizontal, Some(183));
}

#[test]
fn storage_copy_precedes_shared_clamp_and_target_reset_uses_the_same_player_records() {
    let (mut objects, mut world, owner) = fixture();
    world.reflect_all_contacts = Some(false);
    world.targeting_upgrade = Some(TargetingUpgradeState::default());
    world.target_reticle.vertical = None;
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut Callbacks,
        statement_budget: 1,
    };
    host.initialize_player_storage(
        owner,
        PlayerStorageInputs {
            pilot_code: 5,
            reserve_shield: 42,
            score: PlayerScore::from_parts(12345, 9),
        },
    )
    .unwrap();
    let before = *host.world.player(host.objects, owner).unwrap();
    host.reset_player_services(owner).unwrap();
    assert_eq!(*host.world.player(host.objects, owner).unwrap(), before);
    assert_eq!(
        player_storage::get(host.objects, &host.execution.paths.runtime.resources, owner)
            .unwrap()
            .retained_shield,
        42
    );
    assert_eq!(host.world.scene.active_shield, Some(32));
    assert_eq!(host.world.target_reticle.vertical, None);
    host.initialize_player_target(owner).unwrap();
    host.position_and_retain_primary_target().unwrap();
    assert_eq!(host.world.target_reticle.horizontal, Some(100));
    assert_eq!(host.world.target_reticle.vertical, Some(100));
    assert_eq!(
        host.world
            .player(host.objects, owner)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .reserve_shield,
        42
    );
    assert_eq!(host.execution.paths.runtime.resources.owner_count(owner), 1);
}
