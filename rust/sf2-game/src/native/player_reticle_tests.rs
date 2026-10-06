use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::path_scene_state::EncounterObjectiveCounts;
use crate::path_target::{TargetSelection, TargetingUpgradeState};
use crate::player_action::PlayerServiceFlags;
use crate::player_target_lock::{TargetLock, TargetReticle};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId, Vector3};

fn actor(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap()
}

fn setup() -> (ObjectStore, ScenePathWorld, ObjectId, ObjectId, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let proxy = actor(&mut objects);
    let view = actor(&mut objects);
    let mut world = ScenePathWorld::new(RandomState::default());
    world.primary_player = Some(owner);
    world.fixed_players[0] = Some(view);
    world.reticle_enabled = Some(true);
    world.reticle_inhibited = Some(false);
    world.view_transition_mode = Some(ViewTransitionMode::default());
    world.contacts_enabled = Some(false); // overwritten by the live count
    world.objective_counts = Some(EncounterObjectiveCounts {
        remaining_word: 1,
        ..Default::default()
    });
    world.player_service_flags = Some(PlayerServiceFlags::default());
    world.reflect_all_contacts = Some(false);
    world.weapons = Some(crate::weapon_dispatch::WeaponState {
        fallback: Some(proxy),
        ..Default::default()
    });
    world.marker_projection.view_matrix = Some([[32767, 0, 0], [0, 32767, 0], [0, 0, 32767]]);
    world.marker_projection.viewport = Some(ProjectionViewport {
        center: [112, 96],
        left: 0,
        right: 224,
        top: 0,
        bottom: 192,
    });
    world.target_reticle = TargetReticle {
        horizontal: Some(128),
        vertical: Some(128),
    };
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                reticle_display: Some(ReticleDisplay {
                    roll: Angle::from_units(197),
                    mode_flags: TRACK_AIM,
                    display_flags: 0xFF,
                }),
                contact: Some(Default::default()),
                charge: Some(Default::default()),
                rapid_aim: Some(crate::player_rapid::RapidAim {
                    retained_aim: Vector3 {
                        x: 0,
                        y: 0,
                        z: 1000,
                    },
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner, proxy, view)
}

#[test]
fn preparation_covers_all_gate_combinations_and_preserves_nonlinked_roll() {
    let (mut objects, mut world, owner, _, _) = setup();
    objects.get_mut(owner).unwrap().base.roll = Angle::from_units(61);
    for gates in 0..128u8 {
        for linked in 0..4 {
            world.reticle_inhibited = Some(gates & 1 != 0);
            world.view_transition_mode.as_mut().unwrap().flags = if gates & 2 != 0 { 2 } else { 0 };
            world.objective_counts.as_mut().unwrap().remaining_word =
                if gates & 4 != 0 { 0xAB00 } else { 0xCD01 };
            world.player_service_flags = Some(PlayerServiceFlags::from_bits(gates >> 3));
            world.reticle_enabled = Some(gates & 16 == 0);
            world.reflect_all_contacts = Some(gates & 64 != 0);
            let player = world.player_mut(&objects, owner).unwrap();
            player
                .contact
                .as_mut()
                .unwrap()
                .hit
                .hold_secondary_protection = gates & 32 != 0;
            player.charge.as_mut().unwrap().linked_mode = linked & 1 != 0;
            player.charge.as_mut().unwrap().linked_muzzle_disabled = linked & 2 != 0;
            player.reticle_display = Some(ReticleDisplay {
                roll: Angle::from_units(197),
                mode_flags: 0xA7,
                display_flags: 0xFF,
            });
            prepare(&objects, &mut world, owner).unwrap();
            let display = world
                .player(&objects, owner)
                .unwrap()
                .reticle_display
                .unwrap();
            let expected_mode = if gates & 63 != 0 {
                0
            } else if linked == 1 {
                0xC0
            } else {
                0x40
            };
            assert_eq!(display.mode_flags, expected_mode, "{gates} {linked}");
            assert_eq!(display.display_flags, if gates & 64 == 0 { 4 } else { 0 });
            assert_eq!(
                display.roll.units(),
                if expected_mode == 0xC0 { 61 } else { 197 }
            );
        }
    }
}

#[test]
fn early_gate_skips_unpublished_later_inputs_but_still_reads_reflection() {
    let (objects, mut world, owner, _, _) = setup();
    world.reticle_inhibited = Some(true);
    world.view_transition_mode = None;
    world.objective_counts = None;
    world.contacts_enabled = None;
    world.player_service_flags = None;
    world.reticle_enabled = None;
    let player = world.player_mut(&objects, owner).unwrap();
    player.contact = None;
    player.charge = None;
    prepare(&objects, &mut world, owner).unwrap();
    let display = world
        .player(&objects, owner)
        .unwrap()
        .reticle_display
        .unwrap();
    assert_eq!(display.mode_flags, 0);
    assert_eq!(display.display_flags, 4);
    world.reflect_all_contacts = None;
    assert_eq!(
        prepare(&objects, &mut world, owner),
        Err(ReticleError::MissingReflectionMode)
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .reticle_display
            .unwrap()
            .display_flags,
        0
    );
}

#[test]
fn missing_inputs_preserve_completed_preparation_prefix() {
    let (objects, mut world, owner, _, _) = setup();
    world.reticle_inhibited = None;
    assert_eq!(
        prepare(&objects, &mut world, owner),
        Err(ReticleError::MissingInhibition)
    );
    let display = world
        .player(&objects, owner)
        .unwrap()
        .reticle_display
        .unwrap();
    assert_eq!(display.mode_flags, TRACK_AIM);
    assert_eq!(display.display_flags, 0);
    world.reticle_inhibited = Some(false);
    world.reflect_all_contacts = None;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .charge
        .as_mut()
        .unwrap()
        .linked_mode = true;
    assert_eq!(
        prepare(&objects, &mut world, owner),
        Err(ReticleError::MissingReflectionMode)
    );
    let display = world
        .player(&objects, owner)
        .unwrap()
        .reticle_display
        .unwrap();
    assert_eq!(display.mode_flags, 0xC0);
    assert_eq!(display.roll, Angle::ZERO);
}

#[test]
fn inactive_position_resets_both_axes_without_projector_or_previous_samples() {
    let (mut objects, mut world, owner, _, _) = setup();
    world.reticle_enabled = Some(false);
    world.weapons = None;
    world.fixed_players[0] = None;
    world.marker_projection = MarkerProjection::default();
    world.target_reticle = TargetReticle::default();
    world.player_mut(&objects, owner).unwrap().reticle_display = None;
    let before = objects.clone();
    position(&mut objects, &mut world, owner).unwrap();
    assert_eq!(objects, before);
    assert_eq!(
        world.target_reticle,
        TargetReticle {
            horizontal: Some(100),
            vertical: Some(100)
        }
    );
    world.reticle_enabled = Some(true);
    for flags in (0..=u8::MAX).filter(|flags| flags & TRACK_AIM == 0) {
        world.player_mut(&objects, owner).unwrap().reticle_display = Some(ReticleDisplay {
            mode_flags: flags,
            ..Default::default()
        });
        position(&mut objects, &mut world, owner).unwrap();
    }
}

#[test]
fn active_position_copies_current_pose_uses_saved_view_origin_and_retained_matrix() {
    let (mut objects, mut world, owner, proxy, view) = setup();
    objects.get_mut(owner).unwrap().base.roll = Angle::from_units(45);
    objects.get_mut(proxy).unwrap().base.pitch = Angle::from_units(111);
    objects.get_mut(proxy).unwrap().base.yaw = Angle::from_units(51);
    objects.get_mut(view).unwrap().base.position = Vector3 {
        x: 4000,
        y: 5000,
        z: 6000,
    };
    let view_angles = FixedViewAngles {
        pitch: 0x9183,
        yaw: 0xFE21,
        roll: 0x7987,
    };
    view_angles.write_to(objects.get_mut(view).unwrap());
    position(&mut objects, &mut world, owner).unwrap();
    let proxy = &objects.get(proxy).unwrap().base;
    assert_eq!(
        [proxy.pitch, proxy.yaw, proxy.roll],
        [Angle::ZERO, Angle::ZERO, Angle::from_units(45)]
    );
    assert_eq!(
        proxy.position,
        Vector3 {
            x: 0,
            y: 0,
            z: 1000
        }
    );
    assert_eq!(
        world.marker_projection.published_view_angles,
        Some(view_angles)
    );
    assert_eq!(
        world.target_reticle,
        TargetReticle {
            horizontal: Some(132),
            vertical: Some(124)
        }
    );
    // The saved origin changes the screen point; the ordinary position did not.
    objects
        .get_mut(view)
        .unwrap()
        .extension
        .path_state
        .platform_carry
        .saved_position
        .x = 3000;
    position(&mut objects, &mut world, owner).unwrap();
    assert_ne!(world.target_reticle.horizontal, Some(134));
}

#[test]
fn missing_aim_view_matrix_and_vertical_keep_exact_prior_side_effects() {
    let (mut objects, mut world, owner, proxy, view) = setup();
    objects.get_mut(owner).unwrap().base.yaw = Angle::from_units(37);
    let aim = world
        .player_mut(&objects, owner)
        .unwrap()
        .rapid_aim
        .take()
        .unwrap();
    assert_eq!(
        position(&mut objects, &mut world, owner),
        Err(ReticleError::MissingRetainedAim(owner))
    );
    assert_eq!(objects.get(proxy).unwrap().base.yaw.units(), 37);
    assert_eq!(
        objects.get(proxy).unwrap().base.position,
        Vector3::default()
    );
    world.player_mut(&objects, owner).unwrap().rapid_aim = Some(aim);
    world.fixed_players[0] = None;
    assert_eq!(
        position(&mut objects, &mut world, owner),
        Err(ReticleError::MissingView)
    );
    assert_eq!(objects.get(proxy).unwrap().base.position, aim.retained_aim);
    assert_eq!(world.marker_projection.published_view_angles, None);
    world.fixed_players[0] = Some(view);
    let matrix = world.marker_projection.view_matrix.take();
    assert_eq!(
        position(&mut objects, &mut world, owner),
        Err(ReticleError::MissingViewMatrix)
    );
    assert_eq!(
        world.marker_projection.published_view_angles,
        Some(FixedViewAngles::default())
    );
    assert_eq!(world.target_reticle.horizontal, Some(128));
    world.marker_projection.view_matrix = matrix;
    world.target_reticle.vertical = None;
    assert_eq!(
        position(&mut objects, &mut world, owner),
        Err(ReticleError::Position(
            ReticlePositionError::MissingVertical
        ))
    );
    assert_ne!(world.target_reticle.horizontal, Some(128));
}

#[test]
fn player_storage_replacement_initializes_and_replaces_reticle_state() {
    let (mut objects, mut world, owner, _, _) = setup();
    let mut runtime = crate::path_runtime::PathRuntime::default();
    for _ in 0..2 {
        crate::player_storage::replace(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            crate::player_storage::PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 32,
                score: Default::default(),
            },
        )
        .unwrap();
        assert_eq!(
            world.player(&objects, owner).unwrap().reticle_display,
            Some(ReticleDisplay::default())
        );
        world
            .player_mut(&objects, owner)
            .unwrap()
            .reticle_display
            .as_mut()
            .unwrap()
            .mode_flags = 0xC0;
    }
    assert_eq!(world.reticle_enabled, Some(true));
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not an actor visit")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not a death visit")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not a map visit")
    }
}

#[test]
fn scene_composes_real_projection_and_lock_and_latches_partial_failure() {
    let (mut objects, mut world, owner, target, _) = setup();
    world.targeting_upgrade = Some(TargetingUpgradeState { pilot_flags: 0x80 });
    let player = world.player_mut(&objects, owner).unwrap();
    player.auxiliary = Some(SelectedAuxiliaryState {
        mode: 0,
        action_flags: 1,
        stored_world_position: Vector3::default(),
        stored_rotation: Default::default(),
    });
    player.target_selection = Some(TargetSelection {
        candidate: Some(target),
        screen: [112, 96],
        ..Default::default()
    });
    player.target_lock = Some(TargetLock {
        previous_candidate: Some(target),
        ..Default::default()
    });
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        callbacks: &mut callbacks,
        catalog: &catalog,
        statement_budget: 64,
    };
    scene.prepare_player_reticle(owner).unwrap();
    scene.position_and_retain_primary_target().unwrap();
    assert_eq!(
        scene.world.published_homing_target.unwrap().object,
        Some(target)
    );
    assert_eq!(
        scene
            .world
            .player(scene.objects, owner)
            .unwrap()
            .target_lock
            .unwrap()
            .grace_remaining,
        10
    );
    scene.world.target_reticle.vertical = None;
    assert_eq!(
        scene.position_and_retain_primary_target(),
        Err(SceneError::Reticle(ReticleError::Position(
            ReticlePositionError::MissingVertical
        )))
    );
    let retained_horizontal = scene.world.target_reticle.horizontal;
    scene.world.target_reticle.vertical = Some(128);
    assert_eq!(
        scene.position_and_retain_primary_target(),
        Err(SceneError::Faulted)
    );
    assert_eq!(scene.world.target_reticle.horizontal, retained_horizontal);
    assert_eq!(
        scene
            .world
            .player(scene.objects, owner)
            .unwrap()
            .target_lock
            .unwrap()
            .grace_remaining,
        10
    );
}
