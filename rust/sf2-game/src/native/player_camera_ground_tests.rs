use super::*;
#[path = "player_camera_surface_tests.rs"]
mod surface_tests;
#[path = "player_camera_auxiliary_tests.rs"]
mod auxiliary_tests;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::player_storage::PlayerStorage;
use crate::program_state::ProgramData;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::FixedViewAngles;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not dispatched")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not dispatched")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not dispatched")
    }
}

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    owner: ObjectId,
    proxy: ObjectId,
    view: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let owner = allocate();
        let proxy = allocate();
        let view = allocate();
        let mut execution = SceneExecution::default();
        let storage = execution
            .paths
            .runtime
            .resources
            .allocate_owned(
                owner,
                472,
                ProgramData::PlayerStorage(PlayerStorage {
                    fine_pitch: 0,
                    fine_yaw: 0x1273,
                    bank: Angle::ZERO,
                    retained_shield: 71,
                }),
            )
            .unwrap();
        objects.get_mut(owner).unwrap().base.player_storage = Some(storage);
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x31,
                        action_flags: 1,
                        stored_rotation: Default::default(),
                        stored_world_position: Default::default(),
                    }),
                    camera_angles: Some(Default::default()),
                    camera_ground: Some(Default::default()),
                    camera_tracking: Some(Default::default()),
                    camera_position: Some(Default::default()),
                    occupancy_exempt: Some(false),
                    contact: Some(Default::default()),
                    charge: Some(Default::default()),
                    consumable: Some(Default::default()),
                    surface: Some(Default::default()),
                    motion: Some(Default::default()),
                    boundary: Some(Default::default()),
                    steering: Some(Default::default()),
                    pose: Some(Default::default()),
                    ambient: Some(Default::default()),
                    view_distance: Some(Default::default()),
                    vertical: Some(Default::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.fixed_players[0] = Some(view);
        world.environment_plane_height = Some(0);
        world.player_carry_mode = Some(1);
        world.surface_mode = Some(Default::default());
        world.processed_player_input = Some(Default::default());
        world.contacts_enabled = Some(true);
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        Self {
            objects,
            world,
            execution,
            owner,
            proxy,
            view,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn set_angles(&mut self, angles: FixedViewAngles) {
        let r = self.records();
        r.camera_angles
            .as_mut()
            .unwrap()
            .write(&mut r.auxiliary.as_mut().unwrap().stored_rotation, angles);
    }
    fn angles(&self) -> FixedViewAngles {
        let r = self.world.player(&self.objects, self.owner).unwrap();
        r.camera_angles
            .unwrap()
            .capture(r.auxiliary.unwrap().stored_rotation)
    }
    fn ground(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .advance_player_camera_ground(self.owner)
    }
    fn orient(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .advance_player_camera_orientation(self.owner)
    }
}

#[test]
fn height_approach_preserves_wrapped_comparison_and_exact_endpoint() {
    assert_eq!(approach_height(-31, -30), -30);
    assert_eq!(approach_height(-29, -30), -30);
    assert_eq!(approach_height(0, -30), -3);
    assert_eq!(approach_height(i16::MIN, 1), i16::MAX - 2);
}

#[test]
fn frame_bias_retains_full_seven_bit_index_domain() {
    assert_eq!(
        &FRAME_PITCH_BIAS[..16],
        &[128, 128, 0, 0, 0, 0, 128, 128, 128, 128, 0, 0, 0, 0, 128, 128]
    );
    assert_eq!(FRAME_PITCH_BIAS.len(), 128);
}

#[test]
fn protected_ground_resets_only_pitch_and_bob_without_aim_or_environment_services() {
    let mut f = Fixture::new();
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    f.records().camera_ground.as_mut().unwrap().animation_pitch = 419;
    f.records().camera_ground.as_mut().unwrap().height_offset = 33;
    f.set_angles(FixedViewAngles {
        pitch: 0x2375,
        yaw: 0xABD3,
        roll: 0xEF97,
    });
    f.world.weapons = None;
    f.world.fixed_players = [None, None];
    f.world.environment_plane_height = None;
    f.records().consumable = None;
    f.records().motion = None;
    f.execution.paths.runtime.steering.unchanged_axes = 0xA7;
    f.ground().unwrap();
    assert_eq!(
        f.angles(),
        FixedViewAngles {
            pitch: 0,
            yaw: 0xABD3,
            roll: 0xEF97
        }
    );
    assert_eq!(f.records().camera_ground.unwrap().animation_pitch, 0);
    assert_eq!(f.records().camera_ground.unwrap().height_offset, 33);
    assert_eq!(f.execution.paths.runtime.steering.unchanged_axes, 0xA7);
}

#[test]
fn pitch_hold_preserves_reset_prefix_when_fine_angle_owner_is_missing_and_faults_scene() {
    let mut f = Fixture::new();
    f.records().camera_ground.as_mut().unwrap().hold_pitch = true;
    f.records().camera_ground.as_mut().unwrap().animation_pitch = 300;
    f.records().camera_angles = None;
    assert_eq!(
        f.ground(),
        Err(SceneError::PlayerCameraGround(
            GroundCameraError::MissingAngles(f.owner)
        ))
    );
    assert_eq!(f.records().camera_ground.unwrap().animation_pitch, 0);
    assert!(f.execution.is_faulted());
    assert_eq!(f.ground(), Err(SceneError::Faulted));
}

#[test]
fn nonwalker_aim_copies_live_actor_pose_and_ignores_forward_and_player_storage() {
    let mut f = Fixture::new();
    let original = Vector3 {
        x: 313,
        y: -719,
        z: 223,
    };
    let records = *f.world.player(&f.objects, f.owner).unwrap();
    let actor = f.objects.get_mut(f.owner).unwrap();
    actor.base.position = original;
    actor.base.pitch = Angle::from_units(17);
    actor.base.yaw = Angle::from_units(37);
    actor.base.roll = Angle::from_units(79);
    actor.base.player_storage = None;
    f.world.bind_player(&f.objects, f.owner, records).unwrap();
    f.records().boundary = None;
    f.records().camera_ground = None;
    aim_target(
        &mut f.objects,
        &f.world,
        &mut f.execution.paths.runtime,
        f.owner,
        0,
        -128,
        100,
    )
    .unwrap();
    let proxy = &f.objects.get(f.proxy).unwrap().base;
    assert_eq!(
        proxy.position,
        Vector3 {
            x: 313,
            y: -847,
            z: 223
        }
    );
    assert_eq!(
        (proxy.pitch.units(), proxy.yaw.units(), proxy.roll.units()),
        (17, 37, 79)
    );
}

#[test]
fn walker_aim_reads_return_xz_and_distinct_carried_y_leaving_proxy_pitch_roll() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x21;
    f.records().boundary.as_mut().unwrap().return_position = Vector3 {
        x: 137,
        y: -299,
        z: -577,
    };
    f.records()
        .camera_ground
        .as_mut()
        .unwrap()
        .carried_target_height = 413;
    f.objects.get_mut(f.proxy).unwrap().base.pitch = Angle::from_units(77);
    f.objects.get_mut(f.proxy).unwrap().base.roll = Angle::from_units(99);
    aim_target(
        &mut f.objects,
        &f.world,
        &mut f.execution.paths.runtime,
        f.owner,
        0,
        -128,
        0,
    )
    .unwrap();
    let proxy = &f.objects.get(f.proxy).unwrap().base;
    assert_eq!(
        proxy.position,
        Vector3 {
            x: 137,
            y: 285,
            z: -577
        }
    );
    assert_eq!(
        (proxy.pitch.units(), proxy.yaw.units(), proxy.roll.units()),
        (77, 18, 99)
    );
}

#[test]
fn ground_missing_view_keeps_height_proxy_and_aim_control_prefix_before_fault() {
    let mut f = Fixture::new();
    f.records().camera_ground.as_mut().unwrap().height_offset = 0;
    f.world.fixed_players[0] = None;
    f.execution.paths.runtime.steering.unchanged_axes = 0xAB;
    assert_eq!(
        f.ground(),
        Err(SceneError::PlayerCameraGround(
            GroundCameraError::MissingFixedView
        ))
    );
    assert_eq!(f.records().camera_ground.unwrap().height_offset, -3);
    assert_eq!(f.objects.get(f.proxy).unwrap().base.position.y, -3);
    assert_eq!(f.execution.paths.runtime.steering.unchanged_axes, 0);
    assert!(f.execution.is_faulted());
}

#[test]
fn ungrounded_pitch_is_retained_and_bob_cleared_but_target_proxy_still_updates() {
    let mut f = Fixture::new();
    f.set_angles(FixedViewAngles {
        pitch: 0x2375,
        yaw: 0x7DF9,
        roll: 0x1375,
    });
    f.records().camera_ground.as_mut().unwrap().animation_pitch = 19;
    f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
        x: 379,
        y: 719,
        z: -823,
    };
    let view_before = f.objects.get(f.view).unwrap().clone();
    f.ground().unwrap();
    assert_eq!(f.angles().pitch, 0x2375);
    assert_eq!(f.records().camera_ground.unwrap().animation_pitch, 0);
    assert_eq!(
        f.objects.get(f.proxy).unwrap().base.position,
        Vector3 {
            x: 379,
            y: 716,
            z: -823
        }
    );
    assert_eq!(f.objects.get(f.view).unwrap(), &view_before);
}

#[test]
fn unlocked_orientation_decays_lean_and_uses_real_fine_yaw_without_pose_or_input() {
    let mut f = Fixture::new();
    f.records().camera_angles.as_mut().unwrap().yaw_offset = -19;
    f.records().camera_angles.as_mut().unwrap().yaw_difference = 73;
    f.records().contact.as_mut().unwrap().ignores_contacts = true;
    f.records().steering = None;
    f.records().pose = None;
    f.records().charge = None;
    f.world.processed_player_input = None;
    f.set_angles(FixedViewAngles {
        pitch: 0xABCD,
        yaw: 0xFEDC,
        roll: (-3_i16) as u16,
    });
    f.orient().unwrap();
    assert_eq!(
        f.angles(),
        FixedViewAngles {
            pitch: 0xABCD,
            yaw: 0x1273_u16.wrapping_neg(),
            roll: u16::MAX
        }
    );
    assert_eq!(f.records().camera_angles.unwrap().yaw_offset, -17);
    assert_eq!(f.records().camera_angles.unwrap().yaw_difference, 0);
}

#[test]
fn missing_contact_does_not_undo_real_yaw_publication_and_faults_orientation() {
    let mut f = Fixture::new();
    f.set_angles(FixedViewAngles {
        pitch: 1,
        yaw: 2,
        roll: 3,
    });
    f.records().contact = None;
    f.records().camera_angles.as_mut().unwrap().yaw_difference = 299;
    assert_eq!(
        f.orient(),
        Err(SceneError::PlayerCameraCommon(
            crate::player_camera_common::CommonCameraError::World(
                WorldInputError::MissingPlayerContact(f.owner)
            )
        ))
    );
    assert_eq!(
        f.angles(),
        FixedViewAngles {
            pitch: 1,
            yaw: 0x1273_u16.wrapping_neg(),
            roll: 3
        }
    );
    assert_eq!(f.records().camera_angles.unwrap().yaw_difference, 0);
    assert!(f.execution.is_faulted());
}

#[test]
fn common_caller_uses_terrain_pitch_for_surface_family_then_same_orientation_owner() {
    let mut f = Fixture::new();
    f.records().camera_ground.as_mut().unwrap().hold_pitch = true;
    f.set_angles(FixedViewAngles {
        pitch: 400,
        yaw: 500,
        roll: 600,
    });
    f.records().vertical = None;
    // Ground-family height skips vertical tracking; normal distance needs
    // no terrain owner, and ground pitch's hold skips target geometry.
    SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        catalog: &PathCatalog::new(vec![]).unwrap(),
        callbacks: &mut Callbacks,
        statement_budget: 64,
    }
    .advance_player_camera_common(
        f.owner,
        crate::player_camera_tracking::TrackingStyle::Normal,
        false,
    )
    .unwrap();
    assert_eq!(
        f.angles(),
        FixedViewAngles {
            pitch: 0,
            yaw: 0x1273_u16.wrapping_neg(),
            roll: 344
        }
    );
    assert!(!f.execution.is_faulted());
}
