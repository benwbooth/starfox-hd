use super::*;
use crate::path_motion::PublishedPlayerMotion;
use crate::player_action::{PlayerAction, PlayerActionState};
use crate::player_camera_tracking::TrackingStyle;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::positional_audio::{LoopListener, LoopSelection};
use crate::{Angle, Behavior, Object, ObjectKind, ObjectSpawnDefaults, SpatialLoop, Vector3};

fn fixture() -> (
    ObjectStore,
    ScenePathWorld,
    ProgramResources<ProgramData>,
    PositionalAudio,
    ObjectId,
) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(Default::default());
    let mut runtime = crate::path_runtime::PathRuntime::default();
    player_storage::replace(
        &mut objects,
        &mut world,
        &mut runtime,
        owner,
        PlayerStorageInputs {
            pilot_code: 5,
            reserve_shield: 173,
            score: player_storage::PlayerScore::from_parts(59173, 197),
        },
    )
    .unwrap();
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 17,
        run_when_paused: true,
    });
    world.region_registration_count = Some(7);
    world.secondary_region_group = Some(13);
    world.render_environment.ambient_control =
        Some(crate::player_surface_render::AmbientParticleControl::from_bits(0xCDEF));
    world.scene.active_weapon_level = Some(191);
    world.active_consumables = Some(crate::player_visit::PublishedConsumables {
        packed_count: 173,
        kind: 251,
    });
    world.surface_mode = Some(crate::collision_surface::SurfaceMode { flags: 0xE7 });
    world.handoff = Some(crate::path_scene_state::EncounterHandoff {
        player_flags: 255,
        x: 1307,
        z: -9317,
        heading_word: 48139,
    });
    world.weapons = Some(Default::default());
    world.published_motion = Some(PublishedPlayerMotion {
        position: Vector3 {
            x: 591,
            y: -199,
            z: 319,
        },
        delta: Vector3 {
            x: -73,
            y: 91,
            z: -199,
        },
    });
    world.player_mut(&objects, owner).unwrap().action = Some(PlayerActionState {
        action: Some(PlayerAction::ForcedRetreat),
        elapsed: 713,
        auxiliary_counter: 193,
        total_updates: 59171,
    });
    (
        objects,
        world,
        runtime.resources,
        PositionalAudio::default(),
        owner,
    )
}

fn sound(
    objects: &mut ObjectStore,
    positional: &mut PositionalAudio,
    owner: ObjectId,
    z: i16,
) -> LoopSelection {
    let actor = objects.get_mut(owner).unwrap();
    actor.base.position.z = z;
    actor.extension.spatial_loop = SpatialLoop::from_authored_control(5);
    positional.begin_epoch();
    positional
        .observe(
            owner,
            actor,
            false,
            Some(LoopListener {
                position: Vector3::default(),
                bearing: Angle::ZERO,
            }),
        )
        .unwrap();
    positional.pending().unwrap()
}

#[test]
fn entry_tail_clears_action_counters_and_handoff_bits_but_retains_age_motion_and_sound_identity() {
    let (mut objects, mut world, _, mut positional, owner) = fixture();
    let selected = sound(&mut objects, &mut positional, owner, 100);
    positional.publish(false);
    let pending = sound(&mut objects, &mut positional, owner, 3000);
    let motion = world.published_motion;
    let handoff = world.handoff.unwrap();
    let actor_before = objects.get(owner).unwrap().clone();
    for counter in 0..=u16::MAX {
        let records = world.player_mut(&objects, owner).unwrap();
        records.action = Some(PlayerActionState {
            action: Some(PlayerAction::ForcedRetreat),
            elapsed: counter,
            auxiliary_counter: !counter,
            total_updates: counter.rotate_left(5),
        });
        // Each kind of primary camera is exercised by the original-code
        // comparison; this native test only requires a live selector.
        records.camera_dispatch.as_mut().unwrap().style = Some(TrackingStyle::Normal);
        world.handoff.as_mut().unwrap().player_flags = counter as u8;
        world.weapons.as_mut().unwrap().hostile_counts =
            crate::weapon_launch::HostileLaunchCounts {
                collision_disabled: counter as u8,
                aligned_half_plane: (counter >> 8) as u8,
            };
        let actor = objects.get_mut(owner).unwrap();
        actor.base.shape = ShapeId::from_catalog_index(2);
        actor.base.flags.collision_disabled = false;
        finish(&mut objects, &mut world, &mut positional, owner).unwrap();
        assert_eq!(
            world.player(&objects, owner).unwrap().action,
            Some(PlayerActionState {
                action: None,
                elapsed: 0,
                auxiliary_counter: 0,
                total_updates: counter.rotate_left(5),
            })
        );
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .camera_dispatch
                .unwrap()
                .style,
            None
        );
        assert_eq!(world.handoff.unwrap().player_flags, counter as u8 & 0xAF);
        assert_eq!(world.weapons.unwrap().hostile_counts, Default::default());
        assert_eq!(world.published_motion, motion);
    }
    assert_eq!(world.engine_sound_control.unwrap().bits(), CRUISE_SOUND);
    assert_eq!(
        (
            world.handoff.unwrap().x,
            world.handoff.unwrap().z,
            world.handoff.unwrap().heading_word
        ),
        (handoff.x, handoff.z, handoff.heading_word)
    );
    assert_eq!(positional.published_control(), 0);
    assert_eq!(positional.published(), None);
    assert_eq!(positional.retained_selection(), Some(selected));
    assert_eq!(positional.pending(), Some(pending));
    let mut expected = actor_before;
    expected.base.shape = ShapeId::EMPTY;
    expected.base.flags.collision_disabled = true;
    assert_eq!(objects.get(owner).unwrap(), &expected);
}

#[test]
fn reset_composes_real_world_motion_and_action_services_without_releasing_the_player_allocation() {
    let (mut objects, mut world, mut resources, mut positional, owner) = fixture();
    objects
        .get_mut(owner)
        .unwrap()
        .base
        .flags
        .general_search_eligible = true;
    let proxy = world
        .proxies
        .capture_actor(
            &mut objects,
            owner,
            crate::PathCursor {
                path: crate::PathId::from_catalog_index(0),
                command_index: 17,
            },
            &resources,
        )
        .unwrap()
        .unwrap();
    let available = resources.available_capacity();
    let allocation = objects.get(owner).unwrap().base.player_storage;
    let original_age = world
        .player(&objects, owner)
        .unwrap()
        .action
        .unwrap()
        .total_updates;
    let motion = world.published_motion;
    reset(
        &mut objects,
        &mut world,
        &mut resources,
        &mut positional,
        owner,
    )
    .unwrap();
    assert!(world.proxies.get(proxy).is_none());
    assert!(objects.get(owner).unwrap().base.flags.remove_after_tick);
    assert!(objects.get(owner).unwrap().base.flags.collision_disabled);
    assert_eq!(objects.get(owner).unwrap().base.player_storage, allocation);
    assert_eq!(resources.available_capacity(), available);
    assert_eq!(world.region_registration_count, Some(0));
    assert_eq!(world.spawn_defaults.unwrap().group, 255);
    assert_eq!(world.secondary_region_group, Some(255));
    assert_eq!(
        world.occupancy,
        Some(crate::world_occupancy::WorldOccupancy::fully_occupied())
    );
    assert_eq!(
        world.render_environment.ambient_control.unwrap().bits(),
        0xCD00
    );
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.equipment.unwrap().packed_consumables, 173);
    assert_eq!(record.equipment.unwrap().weapon_level, 191);
    assert_eq!(
        record.action.unwrap(),
        PlayerActionState {
            total_updates: original_age,
            ..Default::default()
        }
    );
    assert_eq!(world.published_motion, motion);
}

#[test]
fn tail_failures_preserve_source_order_and_do_not_require_unread_later_inputs() {
    for stage in 0..4 {
        let (mut objects, mut world, _, mut positional, owner) = fixture();
        let selected = sound(&mut objects, &mut positional, owner, 100);
        positional.publish(false);
        objects.get_mut(owner).unwrap().base.shape = ShapeId::from_catalog_index(2);
        objects
            .get_mut(owner)
            .unwrap()
            .base
            .flags
            .collision_disabled = false;
        world
            .player_mut(&objects, owner)
            .unwrap()
            .camera_dispatch
            .as_mut()
            .unwrap()
            .style = Some(TrackingStyle::Normal);
        let error = match stage {
            0 => {
                world.player_mut(&objects, owner).unwrap().action = None;
                EntryResetError::MissingAction(owner)
            }
            1 => {
                world.handoff = None;
                EntryResetError::MissingHandoff
            }
            2 => {
                world.player_mut(&objects, owner).unwrap().camera_dispatch = None;
                EntryResetError::MissingCameraDispatch(owner)
            }
            _ => {
                world.weapons = None;
                EntryResetError::MissingWeaponState
            }
        };
        assert_eq!(
            finish(&mut objects, &mut world, &mut positional, owner),
            Err(error)
        );
        assert_eq!(
            objects.get(owner).unwrap().base.shape,
            if stage == 0 {
                ShapeId::from_catalog_index(2)
            } else {
                ShapeId::EMPTY
            }
        );
        assert_eq!(
            objects.get(owner).unwrap().base.flags.collision_disabled,
            stage >= 2
        );
        assert_eq!(positional.retained_selection(), Some(selected));
        assert_eq!(positional.published().is_some(), stage == 0);
        assert_eq!(world.engine_sound_control.is_some(), stage != 0);
        if stage != 0 {
            assert_eq!(
                world.player(&objects, owner).unwrap().action.unwrap(),
                PlayerActionState {
                    total_updates: 59171,
                    ..Default::default()
                }
            );
        }
    }
}
