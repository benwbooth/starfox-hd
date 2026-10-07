use super::*;
use crate::path_program::PathCatalog;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, ObjectId, ObjectKind, RandomState, ShapeId};

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
    view: ObjectId,
    proxy: ObjectId,
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
        let view = allocate();
        let proxy = allocate();
        let mut world = ScenePathWorld::new(RandomState::default());
        world.fixed_players[0] = Some(view);
        world.view_transition_mode = Some(ViewTransitionMode::default());
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        Self {
            objects,
            world,
            execution: Default::default(),
            view,
            proxy,
        }
    }
    fn actor(&mut self) -> &mut Object {
        self.objects.get_mut(self.view).unwrap()
    }
    fn advance(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .advance_view_blend()
    }
}

#[test]
fn camera_control_aliases_preserve_unrelated_motion_and_share_base_view_snapshots() {
    let mut f = Fixture::new();
    let view = f.actor();
    view.extension.path_state.motion.attached_coordinates = true;
    view.extension.path_state.motion.relative_coordinates = true;
    view.extension.path_state.motion.quadruple_velocity = true;
    view.extension.path_state.motion.refresh_child_chain = true;
    let control = ViewBlendControl {
        capture_position: true,
        capture_rotation: true,
        discard_capture: true,
        position_active: true,
        rotation_active: true,
        fast_position_recovery: true,
    };
    control.write_to(view);
    assert_eq!(ViewBlendControl::capture(view), control);
    assert!(view.extension.path_state.motion.attached_coordinates);
    assert!(view.extension.path_state.motion.relative_coordinates);
    assert!(view.extension.path_state.motion.quadruple_velocity);
    assert!(view.extension.path_state.motion.refresh_child_chain);
    let saved = crate::view_transition::ViewBaseSnapshot::capture(view);
    ViewBlendControl::default().write_to(view);
    view.extension.relative_position.x = 119;
    saved.restore(view);
    assert_eq!(ViewBlendControl::capture(view), control);
    assert_eq!(view.extension.relative_position.x, 119);
}

#[test]
fn missing_proxy_retains_capture_decay_and_angles_but_not_final_snapshot_or_retry() {
    let mut f = Fixture::new();
    f.world.weapons = None;
    let view = f.actor();
    view.base.position.x = 100;
    view.extension.path_state.platform_carry.saved_position.x = 200;
    view.extension.path_state.motion_delta.x = 0x0A77;
    FixedViewAngles {
        pitch: 0x00AB,
        yaw: 0,
        roll: 0,
    }
    .write_to(view);
    ViewBlendControl {
        capture_position: true,
        capture_rotation: true,
        ..Default::default()
    }
    .write_to(view);
    assert_eq!(
        f.advance(),
        Err(SceneError::ViewBlend(ViewBlendError::MissingProxy))
    );
    let view = f.actor();
    assert_eq!(view.extension.relative_position.x, 94);
    assert_eq!(FixedViewAngles::capture(view).pitch, 0x09AB);
    assert_eq!(
        view.extension.path_state.platform_carry.saved_position.x,
        200
    );
    assert_eq!(view.extension.path_state.motion_delta.x, 0x0A77);
    let before = f.objects.clone();
    assert_eq!(f.advance(), Err(SceneError::Faulted));
    assert_eq!(f.objects, before);
}

#[test]
fn scripted_view_clears_position_before_capture_and_discard_needs_an_actual_request() {
    for requested in [false, true] {
        let mut f = Fixture::new();
        f.world
            .view_transition_mode
            .as_mut()
            .unwrap()
            .set_active(true);
        f.world.weapons = None;
        let view = f.actor();
        view.extension.relative_position = Vector3 {
            x: 100,
            y: -200,
            z: 300,
        };
        view.extension.path_state.platform_carry.saved_position.x = 1000;
        ViewBlendControl {
            capture_position: requested,
            discard_capture: true,
            position_active: true,
            ..Default::default()
        }
        .write_to(view);
        f.advance().unwrap();
        assert_eq!(f.actor().extension.relative_position, Vector3::default());
        let control = ViewBlendControl::capture(f.actor());
        assert!(!control.capture_position && !control.position_active);
        assert_eq!(control.discard_capture, !requested);
    }
}

#[test]
fn signed_angular_decay_preserves_fraction_bytes_and_saves_final_not_requested_angles() {
    for value in 0..=u8::MAX {
        let mut f = Fixture::new();
        f.world.weapons = None;
        let view = f.actor();
        FixedViewAngles {
            pitch: 0xF3AB,
            yaw: 0x27CD,
            roll: 0x79EF,
        }
        .write_to(view);
        view.extension.relative_rotation = Rotation {
            pitch: Angle::from_units(value),
            yaw: Angle::from_units(!value),
            roll: Angle::from_units(value.rotate_left(3)),
        };
        ViewBlendControl {
            rotation_active: true,
            ..Default::default()
        }
        .write_to(view);
        f.advance().unwrap();
        let view = f.actor();
        let current = FixedViewAngles::capture(view);
        assert_eq!(
            [current.pitch as u8, current.yaw as u8, current.roll as u8],
            [0xAB, 0xCD, 0xEF]
        );
        assert_eq!(
            high(current.pitch),
            0xF3_u8.wrapping_add(decay_angle(Angle::from_units(value)).units())
        );
        assert_eq!(
            view.extension.path_state.motion_delta,
            Vector3 {
                x: current.pitch as i16,
                y: current.yaw as i16,
                z: current.roll as i16
            }
        );
    }
}

#[test]
fn real_proxy_publishes_current_angles_and_only_yaw_rotates_the_position_offset() {
    let mut f = Fixture::new();
    let view = f.actor();
    FixedViewAngles {
        pitch: 0x39AB,
        yaw: 0x40CD,
        roll: 0x53EF,
    }
    .write_to(view);
    view.base.target_speed = 0;
    view.extension.relative_position = Vector3 {
        x: 32,
        y: -16,
        z: 64,
    };
    ViewBlendControl {
        position_active: true,
        fast_position_recovery: true,
        ..Default::default()
    }
    .write_to(view);
    f.advance().unwrap();
    let proxy = f.objects.get(f.proxy).unwrap();
    assert_eq!(
        [
            proxy.base.pitch.units(),
            proxy.base.yaw.units(),
            proxy.base.roll.units()
        ],
        [0x39, 0xC0, 0x53]
    );
    assert_eq!(proxy.base.position, Vector3::default());
    let (x, z) = rotate_16xz(0xC0, 16, 32);
    assert_eq!(f.actor().base.position, Vector3 { x, y: -8, z });
    assert_eq!(
        f.actor().extension.path_state.platform_carry.saved_position,
        Vector3 { x, y: -8, z }
    );
}

#[test]
fn zero_and_one_unit_offsets_finish_before_proxy_lookup_while_missing_mode_precedes_writes() {
    let mut f = Fixture::new();
    f.world.weapons = None;
    let view = f.actor();
    view.extension.relative_position = Vector3 { x: 1, y: -1, z: 0 };
    ViewBlendControl {
        position_active: true,
        ..Default::default()
    }
    .write_to(view);
    f.advance().unwrap();
    assert!(!ViewBlendControl::capture(f.actor()).position_active);
    f.world.view_transition_mode = None;
    let before = f.objects.clone();
    assert_eq!(
        f.advance(),
        Err(SceneError::ViewBlend(ViewBlendError::MissingViewMode))
    );
    assert_eq!(f.objects, before);
}
