use super::{Callbacks, Fixture};
#[path = "player_camera_dispatch_tests.rs"]
mod dispatch_tests;
use crate::path_motion::PublishedPlayerMotion;
use crate::path_program::PathCatalog;
use crate::path_scene_state::{EncounterCameraFocus, EncounterHandoff};
use crate::player_camera_auxiliary::{
    self as camera, AuxiliaryCameraError as Error, AuxiliaryCameraTask as Task, OrbitStyle as Style,
};
use crate::scene_strategy::{SceneActors, SceneError};
use crate::view_blend::ViewBlendControl;
use crate::view_transition::FixedViewAngles;
use crate::{Angle, Vector3};

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.records().camera_auxiliary = Some(Default::default());
    f.records().flight_displacement = Some(Vector3 {
        x: 17,
        y: -11,
        z: 23,
    });
    f.world.scene.player_configuration = Some(0);
    f.world.handoff = Some(EncounterHandoff {
        x: 100,
        z: 500,
        heading_word: 0xE700,
        player_flags: 0xC3,
    });
    f.world.camera_focus = Some(EncounterCameraFocus {
        position: Vector3 {
            x: 1000,
            y: -199,
            z: 4000,
        },
    });
    f.world.published_motion = Some(PublishedPlayerMotion {
        position: Vector3 {
            x: 100,
            y: -200,
            z: 300,
        },
        delta: Default::default(),
    });
    f.world.encounter_timer_steps = Some(0);
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
fn no_task_increments_word_counter_without_unrelated_dependencies() {
    let mut f = fixture();
    f.world.weapons = None;
    f.world.camera_focus = None;
    f.world.published_motion = None;
    f.world.scene.player_configuration = None;
    f.objects
        .get_mut(f.view)
        .unwrap()
        .extension
        .path_state
        .script_value = u16::MAX;
    advance(&mut f).unwrap();
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0
    );
    assert_eq!(f.world.camera_tracking, None);
}

#[test]
fn initialization_runs_task_same_visit_and_copies_velocity_without_integration() {
    let mut f = fixture();
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.records().camera_auxiliary.as_mut().unwrap().task = Task::Initialize(Style::Retreat);
    FixedViewAngles {
        pitch: 0x1234,
        yaw: 0x3477,
        roll: 0x8951,
    }
    .write_to(f.objects.get_mut(f.view).unwrap());
    f.objects
        .get_mut(f.view)
        .unwrap()
        .extension
        .path_state
        .script_value = 199;
    advance(&mut f).unwrap();
    let state = f.records().camera_auxiliary.unwrap();
    assert_eq!(state.task, Task::Orbit(Style::Retreat));
    assert_eq!(state.retreat_distance, -85);
    assert_eq!(f.world.camera_tracking.unwrap().actor, Some(f.owner));
    let view = f.objects.get(f.view).unwrap();
    assert_eq!(view.extension.path_state.script_value, 0);
    assert_eq!(view.extension.path_state.script_parameter, 0xCB);
    assert_eq!(
        view.base.velocity,
        Vector3 {
            x: 17,
            y: -11,
            z: 23
        }
    );
    assert_eq!(
        view.base.position,
        f.objects.get(f.proxy).unwrap().base.position
    );
    assert_eq!(FixedViewAngles::capture(view).roll, 0);
    advance(&mut f).unwrap();
    assert_eq!(f.records().camera_auxiliary.unwrap().retreat_distance, -90);
    assert_eq!(
        f.objects
            .get(f.view)
            .unwrap()
            .extension
            .path_state
            .script_value,
        1
    );
}

#[test]
fn ground_retreat_retains_decreasing_distance_without_using_it_for_placement() {
    // Original cosine(0) is 127/128, with magnitude truncation: even a
    // zero-angle projection is not the unscaled forward input.
    let mut f = fixture();
    f.world.scene.player_configuration = Some(9);
    f.records().auxiliary.as_mut().unwrap().mode = 0x21;
    f.records().camera_auxiliary.as_mut().unwrap().task = Task::Initialize(Style::Retreat);
    advance(&mut f).unwrap();
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        Vector3 {
            x: 0,
            y: -40,
            z: -158
        }
    );
    advance(&mut f).unwrap();
    assert_eq!(f.records().camera_auxiliary.unwrap().retreat_distance, -90);
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        Vector3 {
            x: 0,
            y: -40,
            z: -158
        }
    );
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    advance(&mut f).unwrap();
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        Vector3 {
            x: 0,
            y: -50,
            z: -94
        }
    );
}

#[test]
fn focus_uses_published_position_and_full_timer_without_replacing_live_actor() {
    let mut f = fixture();
    f.records().camera_auxiliary.as_mut().unwrap().task = Task::Initialize(Style::EncounterFocus);
    f.objects.get_mut(f.owner).unwrap().base.position = Vector3 {
        x: -500,
        y: 1777,
        z: 900,
    };
    advance(&mut f).unwrap();
    assert_eq!(
        f.objects.get(f.proxy).unwrap().base.position,
        Vector3 {
            x: 100,
            y: -200,
            z: 300
        }
    );
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        Vector3 {
            x: 100,
            y: -180,
            z: 181
        }
    );
    f.world.encounter_timer_steps = Some(256);
    advance(&mut f).unwrap();
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        Vector3 {
            x: 100,
            y: -250,
            z: 152
        }
    );
    assert_eq!(
        f.objects.get(f.owner).unwrap().base.position,
        Vector3 {
            x: -500,
            y: 1777,
            z: 900
        }
    );
}

#[test]
fn handoff_uses_previous_view_position_and_preserves_full_heading_companion() {
    let mut f = fixture();
    f.records().camera_auxiliary.as_mut().unwrap().task = Task::Handoff;
    f.objects.get_mut(f.view).unwrap().base.position = Vector3 {
        x: 12345,
        y: -299,
        z: -16000,
    };
    advance(&mut f).unwrap();
    let proxy = f.objects.get(f.proxy).unwrap();
    assert_eq!(
        proxy.base.position,
        Vector3 {
            x: 100,
            y: 0,
            z: 500
        }
    );
    assert_eq!(proxy.base.yaw, Angle::ZERO);
    assert_eq!(proxy.extension.path_state.repeat_counter, 0xE7);
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        Vector3 {
            x: 6,
            y: -15,
            z: 90
        }
    );
    assert_eq!(f.world.camera_tracking.unwrap().actor, Some(f.proxy));
    assert_eq!(
        f.objects.get(f.view).unwrap().base.velocity,
        Vector3::default()
    );
    assert_eq!(f.execution.paths.runtime.steering.unchanged_axes, 0);
}

#[test]
fn installers_capture_except_handoff_and_preserve_other_blend_controls() {
    let mut f = fixture();
    let original = ViewBlendControl {
        discard_capture: true,
        position_active: true,
        rotation_active: true,
        fast_position_recovery: true,
        ..Default::default()
    };
    original.write_to(f.objects.get_mut(f.view).unwrap());
    camera::install(&mut f.objects, &mut f.world, f.owner, Task::Handoff).unwrap();
    assert_eq!(
        ViewBlendControl::capture(f.objects.get(f.view).unwrap()),
        original
    );
    camera::install(&mut f.objects, &mut f.world, f.owner, Task::None).unwrap();
    assert_eq!(
        ViewBlendControl::capture(f.objects.get(f.view).unwrap()),
        ViewBlendControl {
            capture_position: true,
            capture_rotation: true,
            ..original
        }
    );
}

#[test]
fn missing_focus_keeps_initialized_task_counter_tracking_and_velocity_prefix() {
    let mut f = fixture();
    f.world.camera_focus = None;
    f.records().camera_auxiliary.as_mut().unwrap().task = Task::Initialize(Style::EncounterFocus);
    let mut callbacks = Callbacks;
    let catalog = PathCatalog::new(vec![]).unwrap();
    let owner = f.owner;
    let mut scene = SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(
        scene.advance_player_camera_auxiliary(owner),
        Err(SceneError::PlayerCameraAuxiliary(Error::MissingFocus))
    );
    assert_eq!(
        scene.advance_player_camera_auxiliary(owner),
        Err(SceneError::Faulted)
    );
    assert!(f.execution.is_faulted());
    assert_eq!(
        f.records().camera_auxiliary.unwrap().task,
        Task::Orbit(Style::EncounterFocus)
    );
    assert_eq!(f.world.camera_tracking.unwrap().actor, Some(owner));
    assert_eq!(
        f.objects.get(f.view).unwrap().base.velocity,
        Vector3 {
            x: 17,
            y: -11,
            z: 23
        }
    );
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
fn missing_surface_mode_retains_retreat_orbit_proxy_and_initial_pitch_prefix() {
    let mut f = fixture();
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    f.records().camera_auxiliary.as_mut().unwrap().task = Task::Initialize(Style::Retreat);
    f.world.surface_mode = None;
    assert_eq!(advance(&mut f), Err(Error::MissingSurfaceMode));
    assert_eq!(f.records().camera_auxiliary.unwrap().retreat_distance, -85);
    assert_eq!(
        f.objects.get(f.view).unwrap().base.position,
        f.objects.get(f.proxy).unwrap().base.position
    );
    assert_eq!(
        FixedViewAngles::capture(f.objects.get(f.view).unwrap()).pitch,
        (-2560_i16) as u16
    );
    assert_eq!(
        f.objects.get(f.view).unwrap().base.velocity,
        Vector3::default()
    );
}
