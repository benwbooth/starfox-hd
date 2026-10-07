use super::super::{Callbacks, Fixture};
use crate::path_program::PathCatalog;
use crate::player_camera_auxiliary::AuxiliaryCameraTask;
use crate::player_camera_dispatch::{self as camera, CameraDispatchError as Error};
use crate::player_camera_tracking::TrackingStyle;
use crate::player_view_distance::ViewDistanceError;
use crate::scene_strategy::{SceneActors, SceneError};
use crate::view_transition::FixedViewAngles;
use crate::world_occupancy::{MarkerCoverage, OccupancyChange, WorldOccupancy, WorldRectangle};
use crate::Vector3;

fn fixture(style: Option<TrackingStyle>) -> Fixture {
    let mut f = super::fixture();
    f.records().camera_dispatch = Some(crate::player_camera_dispatch::PlayerCameraDispatch {
        style,
        ..Default::default()
    });
    f.records().camera_surface = Some(Default::default());
    f.records().mode_selection = Some(Default::default());
    f.world.view_transition_mode = Some(Default::default());
    f.world.action_gate = Some(Default::default());
    f.world.camera_height_limits = Some((-600, 0));
    f.world.camera_projection_offset = Some(99);
    f.world.horizon_disabled = Some(false);
    f.world.player_view_options_enabled = Some(true);
    f.world.scene.player_view_control = Some(0);
    f.world.occupancy = Some(WorldOccupancy::default());
    f
}
fn advance(f: &mut Fixture) -> Result<(), Error> {
    camera::advance(
        &mut f.objects,
        &mut f.world,
        &mut f.execution.paths.runtime,
        f.owner,
    )
}

#[test]
fn scripted_view_only_publishes_existing_roll_without_player_or_action_services() {
    let mut f = fixture(Some(TrackingStyle::Surface));
    f.world
        .view_transition_mode
        .as_mut()
        .unwrap()
        .set_active(true);
    f.world.action_gate = None;
    f.world.release_player_bindings(f.owner);
    FixedViewAngles {
        pitch: 0x1234,
        yaw: 0x8937,
        roll: 0xABCD,
    }
    .write_to(f.objects.get_mut(f.view).unwrap());
    f.objects
        .get_mut(f.view)
        .unwrap()
        .extension
        .path_state
        .script_value = 71;
    advance(&mut f).unwrap();
    assert_eq!(f.world.published_camera_roll, Some(0xABCD));
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .script_value,
        71
    );
}

#[test]
fn absent_primary_task_skips_publication_and_footer_but_action_gate_runs_footer() {
    let mut f = fixture(None);
    f.world.published_camera_roll = Some(7917);
    f.objects
        .get_mut(f.view)
        .unwrap()
        .extension
        .path_state
        .script_value = 999;
    f.records().camera_auxiliary.as_mut().unwrap().task = AuxiliaryCameraTask::Handoff;
    advance(&mut f).unwrap();
    assert_eq!(f.world.published_camera_roll, Some(7917));
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .script_value,
        999
    );
    f.world.action_gate.as_mut().unwrap().code = 1;
    f.records().camera_dispatch = None;
    advance(&mut f).unwrap();
    assert_eq!(f.world.published_camera_roll, Some(0));
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .script_value,
        1000
    );
    assert_eq!(f.world.camera_tracking.unwrap().actor, Some(f.proxy));
}

#[test]
fn projection_midpoint_rounds_toward_zero_then_signed_floor_and_clamp() {
    let mut f = fixture(Some(TrackingStyle::ProjectionCorrected));
    for (top, bottom, height, expected) in [
        (-3, 0, -2, -1),
        (32767, 1, 0, 160),
        (-3, 0, 32767, -160),
        (1, 2, 16, 0),
    ] {
        f.world.camera_height_limits = Some((top, bottom));
        f.objects.get_mut(f.view).unwrap().base.position.y = height;
        camera::advance_projection(&f.objects, &mut f.world, f.owner).unwrap();
        assert_eq!(f.world.camera_projection_offset, Some(expected));
    }
    f.records()
        .camera_dispatch
        .as_mut()
        .unwrap()
        .projection_correction_disabled = true;
    f.world.camera_height_limits = None;
    f.world.camera_projection_offset = Some(-77);
    camera::advance_projection(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(f.world.camera_projection_offset, Some(-77));
}

#[test]
fn missing_view_selection_preserves_different_mode_ordering() {
    for style in [TrackingStyle::Normal, TrackingStyle::Surface] {
        let mut f = fixture(Some(style));
        f.world.player_view_options_enabled = None;
        f.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .camera_pitch_recoil = 9;
        let previous = FixedViewAngles::capture(f.objects.get(f.view).unwrap());
        assert_eq!(
            advance(&mut f),
            Err(Error::Distance(ViewDistanceError::MissingViewOptions))
        );
        if style == TrackingStyle::Normal {
            assert_ne!(
                FixedViewAngles::capture(f.objects.get(f.view).unwrap()),
                previous
            );
            assert_ne!(f.records().contact.unwrap().hit.camera_pitch_recoil, 9);
            assert!(f.world.published_camera_roll.is_some());
        } else {
            assert_eq!(
                FixedViewAngles::capture(f.objects.get(f.view).unwrap()),
                previous
            );
            assert_eq!(f.records().contact.unwrap().hit.camera_pitch_recoil, 9);
            assert_eq!(f.world.published_camera_roll, None);
        }
        assert_eq!(
            f.objects
                .get(f.view)
                .unwrap()
                .extension
                .path_state
                .script_value,
            0
        );
    }
}

#[test]
fn surface_occupancy_samples_entering_view_before_publishing_new_position() {
    let mut f = fixture(Some(TrackingStyle::Surface));
    f.records().auxiliary.as_mut().unwrap().mode = 0x21;
    f.records().boundary.as_mut().unwrap().return_position = Vector3 {
        x: 4096,
        y: 0,
        z: 4096,
    };
    let marker = MarkerCoverage::from_rectangle(WorldRectangle {
        x: 0,
        z: 0,
        width: 1,
        depth: 1,
    })
    .unwrap();
    f.world
        .occupancy
        .as_mut()
        .unwrap()
        .apply(&marker, OccupancyChange::Mark);
    advance(&mut f).unwrap();
    assert!(!f.records().camera_dispatch.unwrap().outside_occupied_world);
    let view = f.objects.get(f.view).unwrap();
    assert!(!f
        .world
        .occupancy
        .as_ref()
        .unwrap()
        .contains(view.base.position));
    advance(&mut f).unwrap();
    assert!(f.records().camera_dispatch.unwrap().outside_occupied_world);
    f.records().occupancy_exempt = Some(true);
    f.world.occupancy = None;
    advance(&mut f).unwrap();
    assert!(!f.records().camera_dispatch.unwrap().outside_occupied_world);
}

#[test]
fn horizon_requests_are_sticky_and_auxiliary_does_not_republish_changed_roll() {
    let mut f = fixture(Some(TrackingStyle::Normal));
    f.records().mode_selection.as_mut().unwrap().surface_control = 4;
    f.records().contact.as_mut().unwrap().ignores_contacts = true;
    let r = f.records();
    r.camera_angles.as_mut().unwrap().write(
        &mut r.auxiliary.as_mut().unwrap().stored_rotation,
        FixedViewAngles {
            pitch: 0,
            yaw: 0,
            roll: 20,
        },
    );
    f.records().camera_auxiliary.as_mut().unwrap().task = AuxiliaryCameraTask::Handoff;
    advance(&mut f).unwrap();
    assert_eq!(f.world.horizon_disabled, Some(true));
    assert_eq!(f.world.published_camera_roll, Some(10));
    assert_eq!(
        FixedViewAngles::capture(f.objects.get(f.view).unwrap()).roll,
        0
    );
    f.records().mode_selection.as_mut().unwrap().surface_control = 0;
    advance(&mut f).unwrap();
    assert_eq!(f.world.horizon_disabled, Some(true));
}

#[test]
fn missing_surface_occupancy_latches_scene_after_preparation_and_recoil_before_publication() {
    let mut f = fixture(Some(TrackingStyle::Surface));
    f.world.occupancy = None;
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .camera_pitch_recoil = 19;
    let mut callbacks = Callbacks;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut scene = SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.advance_player_camera(f.owner),
        Err(SceneError::PlayerCameraDispatch(Error::MissingOccupancy))
    );
    assert_eq!(
        scene.advance_player_camera(f.owner),
        Err(SceneError::Faulted)
    );
    assert!(f.execution.is_faulted());
    assert_ne!(f.records().contact.unwrap().hit.camera_pitch_recoil, 19);
    assert_eq!(f.world.published_camera_roll, None);
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0
    );
}

#[test]
fn paired_view_update_saves_history_before_normal_plane_limit() {
    let mut f = fixture(Some(TrackingStyle::Normal));
    camera::advance_with_continuity(
        &mut f.objects,
        &mut f.world,
        &mut f.execution.paths.runtime,
        f.owner,
    )
    .unwrap();
    let previous = f.objects.get(f.view).unwrap().base.position;
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .platform_carry
            .saved_position,
        previous
    );
    f.world.environment_plane_height = Some(previous.y.wrapping_sub(100));
    camera::clamp_normal_to_plane(&mut f.objects, &f.world, f.owner).unwrap();
    let view = f.objects.get(f.view).unwrap();
    assert_eq!(view.base.position.y, previous.y.wrapping_sub(110));
    assert_eq!(
        view.extension.path_state.platform_carry.saved_position,
        previous
    );
}

#[test]
fn free_flight_camera_installation_respects_existing_tasks_and_requests_real_capture() {
    use crate::view_blend::ViewBlendControl;
    let mut f = fixture(Some(TrackingStyle::Surface));
    f.records().mode_selection.as_mut().unwrap().surface_control = 0xBD;
    f.records().camera_auxiliary.as_mut().unwrap().task = AuxiliaryCameraTask::Handoff;
    camera::select_free_flight_camera(&mut f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(
        f.records().camera_dispatch.unwrap().style,
        Some(TrackingStyle::Surface)
    );
    assert!(!ViewBlendControl::capture(f.objects.get(f.view).unwrap()).capture_position);
    f.records().camera_auxiliary.as_mut().unwrap().task = AuxiliaryCameraTask::None;
    camera::select_free_flight_camera(&mut f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(
        f.records().camera_dispatch.unwrap().style,
        Some(TrackingStyle::Normal)
    );
    assert_eq!(f.records().mode_selection.unwrap().surface_control, 0xB5);
    let control = ViewBlendControl::capture(f.objects.get(f.view).unwrap());
    assert!(control.capture_position && control.capture_rotation);
    f.records().view_distance.as_mut().unwrap().distance = -999;
    f.records().camera_auxiliary = None;
    camera::select_free_flight_camera(&mut f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(f.records().view_distance.unwrap().distance, -999);
}

#[test]
fn map_exemption_has_one_owner_for_camera_player_and_projectile_consumers() {
    use crate::path_control::PlayerTarget;
    use crate::path_invocation::InvocationWorld;
    use crate::player_occupancy::{self, OccupancyContext};
    let mut f = fixture(Some(TrackingStyle::Surface));
    f.world.primary_player = Some(f.owner);
    f.world.occupancy = None;
    f.records().occupancy_exempt = Some(true);
    f.records().motion.as_mut().unwrap().contact_flags = 0x80;
    assert_eq!(
        f.world
            .path_world(&f.objects, f.owner, PlayerTarget::Primary)
            .unwrap()
            .selected_occupancy_exempt,
        Some(true)
    );
    player_occupancy::advance(
        &mut f.objects,
        &mut f.world,
        &mut f.execution.paths.runtime.resources,
        f.owner,
        OccupancyContext::default(),
    )
    .unwrap();
    assert_eq!(f.records().motion.unwrap().contact_flags, 0);
    advance(&mut f).unwrap();
    assert!(!f.records().camera_dispatch.unwrap().outside_occupied_world);
    f.records().occupancy_exempt = Some(false);
    assert_eq!(
        f.world
            .path_world(&f.objects, f.owner, PlayerTarget::Primary)
            .unwrap()
            .selected_occupancy_exempt,
        Some(false)
    );
    assert_eq!(advance(&mut f), Err(Error::MissingOccupancy));
}
