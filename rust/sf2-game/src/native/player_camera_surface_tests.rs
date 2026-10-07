use super::*;
use crate::player_camera_surface::SurfaceCameraError;
use crate::player_camera_tracking::TrackingStyle;

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.records().camera_surface = Some(Default::default());
    f.world.scene.player_configuration = Some(9);
    f
}
fn height(f: &mut Fixture) -> Result<(), SceneError<()>> {
    SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        catalog: &PathCatalog::new(vec![]).unwrap(),
        callbacks: &mut Callbacks,
        statement_budget: 64,
    }
    .advance_player_camera_surface_height(f.owner, TrackingStyle::Surface, false)
}
fn current(f: &mut Fixture) -> i16 {
    f.records().auxiliary.unwrap().stored_world_position.y
}

#[test]
fn protected_contact_exemption_uses_carried_height_without_environment_or_surface_query() {
    let mut f = fixture();
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    f.records().contact.as_mut().unwrap().ignores_contacts = true;
    f.records()
        .auxiliary
        .as_mut()
        .unwrap()
        .stored_world_position
        .y = -31;
    f.records()
        .camera_ground
        .as_mut()
        .unwrap()
        .carried_target_height = 100;
    f.records().motion = None;
    f.records().surface = None;
    f.world.environment_plane_height = None;
    f.world.player_carry_mode = None;
    height(&mut f).unwrap();
    assert_eq!(current(&mut f), 1);
    assert!(f.records().contact.unwrap().hit.hold_secondary_protection);
}

#[test]
fn standing_plane_follow_clears_ground_and_recovery_controls_before_flight_height() {
    let mut f = fixture();
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.records()
        .auxiliary
        .as_mut()
        .unwrap()
        .stored_world_position
        .y = 100;
    f.records()
        .camera_ground
        .as_mut()
        .unwrap()
        .follow_environment_plane = true;
    f.records().camera_ground.as_mut().unwrap().hold_pitch = true;
    f.records().consumable.as_mut().unwrap().recovery_blocked = true;
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .base
        .flags
        .standing_on_surface = true;
    f.world.environment_plane_height = None;
    height(&mut f).unwrap();
    assert_eq!(current(&mut f), 44);
    assert!(!f.records().camera_ground.unwrap().follow_environment_plane);
    assert!(!f.records().camera_ground.unwrap().hold_pitch);
    assert!(!f.records().consumable.unwrap().recovery_blocked);
}

#[test]
fn carried_recovery_reaches_lower_plane_then_holds_pitch_until_upper_plane_visit() {
    let mut f = fixture();
    f.records().consumable.as_mut().unwrap().recovery_blocked = true;
    f.records()
        .auxiliary
        .as_mut()
        .unwrap()
        .stored_world_position
        .y = -16;
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .carry_selected_player = true;
    height(&mut f).unwrap();
    assert_eq!(current(&mut f), -12);
    assert!(f.records().camera_ground.unwrap().hold_pitch);
    for _ in 0..7 {
        height(&mut f).unwrap();
    }
    assert_eq!(current(&mut f), 16);
    assert!(f.records().camera_ground.unwrap().hold_pitch);
    height(&mut f).unwrap();
    assert!(!f.records().camera_ground.unwrap().hold_pitch);
    assert!(!f.records().consumable.unwrap().recovery_blocked);
}

#[test]
fn protected_uncarried_actor_crosses_plane_then_clears_only_protection_and_transition() {
    let mut f = fixture();
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    f.objects.get_mut(f.owner).unwrap().base.position.y = -100;
    height(&mut f).unwrap();
    assert_eq!(current(&mut f), 12);
    assert_eq!(f.records().camera_tracking.unwrap().anchor_height, -100);
    assert!(f.records().camera_surface.unwrap().returning_below_plane);
    for _ in 0..7 {
        height(&mut f).unwrap();
    }
    assert_eq!(current(&mut f), -16);
    assert!(f.records().contact.unwrap().hit.hold_secondary_protection);
    height(&mut f).unwrap();
    assert!(!f.records().camera_surface.unwrap().returning_below_plane);
    assert!(!f.records().contact.unwrap().hit.hold_secondary_protection);
    assert_eq!(f.objects.get(f.owner).unwrap().base.position.y, -100);
}

#[test]
fn linked_height_uses_bounded_half_then_optional_eighth_response_without_plane_follow() {
    for (disabled, expected) in [(false, 40), (true, 5)] {
        let mut f = fixture();
        f.records().charge.as_mut().unwrap().linked_mode = true;
        f.records().charge.as_mut().unwrap().linked_muzzle_disabled = disabled;
        f.records()
            .camera_ground
            .as_mut()
            .unwrap()
            .follow_environment_plane = true;
        f.records()
            .camera_ground
            .as_mut()
            .unwrap()
            .carried_target_height = 100;
        f.world.surface_mode.as_mut().unwrap().flags = 2;
        f.world.environment_plane_height = None;
        height(&mut f).unwrap();
        assert_eq!(current(&mut f), expected);
        assert!(f.records().camera_ground.unwrap().follow_environment_plane);
    }
}

#[test]
fn missing_plane_preserves_initial_follow_height_and_faults_surface_service() {
    let mut f = fixture();
    f.records()
        .camera_ground
        .as_mut()
        .unwrap()
        .follow_environment_plane = true;
    f.records().camera_ground.as_mut().unwrap().hold_pitch = true;
    f.records()
        .auxiliary
        .as_mut()
        .unwrap()
        .stored_world_position
        .y = 100;
    f.world.environment_plane_height = None;
    assert_eq!(
        height(&mut f),
        Err(SceneError::PlayerCameraSurface(
            SurfaceCameraError::MissingEnvironmentPlane
        ))
    );
    assert_eq!(current(&mut f), 50);
    assert!(f.execution.is_faulted());
    assert_eq!(height(&mut f), Err(SceneError::Faulted));
}

#[test]
fn surface_placement_publishes_yaw_before_decay_and_only_uses_return_xz() {
    let mut f = fixture();
    f.records()
        .auxiliary
        .as_mut()
        .unwrap()
        .stored_world_position
        .y = 91;
    f.records().camera_ground.as_mut().unwrap().hold_pitch = true;
    f.records().boundary.as_mut().unwrap().return_position = Vector3 {
        x: 7,
        y: 300,
        z: 19,
    };
    f.records().camera_angles.as_mut().unwrap().yaw_difference = 20;
    f.records().camera_angles.as_mut().unwrap().yaw_offset = 8;
    f.set_angles(FixedViewAngles {
        pitch: 0,
        yaw: 97,
        roll: 600,
    });
    SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        catalog: &PathCatalog::new(vec![]).unwrap(),
        callbacks: &mut Callbacks,
        statement_budget: 64,
    }
    .prepare_player_camera_surface(f.owner, TrackingStyle::Surface, false)
    .unwrap();
    assert_eq!(
        f.records().auxiliary.unwrap().stored_world_position,
        Vector3 { x: 7, y: 87, z: 19 }
    );
    assert_eq!(
        f.angles(),
        FixedViewAngles {
            pitch: 0,
            yaw: 0x1273_u16.wrapping_neg().wrapping_sub(20),
            roll: 300
        }
    );
    assert_eq!(f.records().camera_angles.unwrap().yaw_difference, 0);
    assert_eq!(f.records().camera_angles.unwrap().yaw_offset, 7);
}
