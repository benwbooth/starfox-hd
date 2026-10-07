use super::*;
use crate::path_program::PathCatalog;
use crate::scene_path_world::ScenePathWorld;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

fn inputs() -> SurfaceMotionInputs {
    SurfaceMotionInputs {
        search: SurfaceSearch::Full,
        strategy_tick: 0,
        gravity: None,
        inherited_tilt: SurfaceTilt {
            pitch: Angle::from_units(37),
            roll: Angle::from_units(19),
        },
    }
}

fn actor(objects: &mut ObjectStore) -> ObjectId {
    objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap()
}

#[test]
fn dead_zone_is_asymmetric_and_gravity_follows_position_integration() {
    for value in i16::MIN..=i16::MAX {
        assert_eq!(
            dead_zone(value),
            if (-3..3).contains(&value) { 0 } else { value }
        );
    }
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let object = objects.get_mut(owner).unwrap();
    object.base.velocity = Vector3 { x: -3, y: 3, z: 2 };
    object.extension.path_state.platform_carry.saved_position = Vector3 { x: 1, y: -1, z: 5 };
    let result = advance(
        &mut objects,
        owner,
        SurfaceMotionInputs {
            gravity: Some(5),
            ..inputs()
        },
    )
    .unwrap();
    let object = objects.get(owner).unwrap();
    assert_eq!(object.base.position, Vector3 { x: 1, y: 2, z: 5 });
    assert_eq!(object.base.velocity.y, 8);
    assert!(!object.base.flags.standing_on_surface);
    assert_eq!(result.inherited_tilt, SurfaceTilt::default());
}

#[test]
fn inherited_delta_is_word_scaled_and_vertical_carry_is_not_divided_back() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    for grounded in [false, true] {
        let object = objects.get_mut(owner).unwrap();
        object.base.position = Vector3::default();
        object.base.velocity = Vector3::default();
        object.extension.path_state.platform_carry.saved_position = Vector3::default();
        object.extension.path_state.motion_delta = Vector3 {
            x: 8193,
            y: -8193,
            z: -8193,
        };
        advance(
            &mut objects,
            owner,
            SurfaceMotionInputs {
                search: if grounded {
                    SurfaceSearch::Reduced
                } else {
                    SurfaceSearch::Full
                },
                ..inputs()
            },
        )
        .unwrap();
        let object = objects.get(owner).unwrap();
        assert_eq!(object.base.flags.standing_on_surface, grounded);
        assert_eq!(
            object.extension.path_state.platform_carry.saved_position,
            // The zero-angle Q15 matrix has 32767, not 32768, on its
            // diagonal: +8 becomes +7 while -8 stays -8 before division.
            Vector3 {
                x: if grounded { 0 } else { 1 },
                y: -8,
                z: -1
            }
        );
        assert_eq!(object.extension.path_state.motion_delta, Vector3::default());
    }
}

#[test]
fn footprint_ties_keep_negative_x_and_polygon_requires_an_edge() {
    let geometry = SurfaceGeometry {
        footprint: SurfaceFootprint::Rectangle {
            negative_x: -10,
            negative_z: -10,
            positive_x: 10,
            positive_z: 10,
        },
        ..Default::default()
    };
    assert_eq!(
        escape(geometry, Vector3::default()).unwrap(),
        Vector3 { x: -10, y: 0, z: 0 }
    );
    assert_eq!(
        polygon_escape(&[], 0, [0, 0]),
        Err(SurfaceMotionError::InvalidPolygon)
    );
    assert_eq!(
        polygon_escape(&[[0, 0]], 0, [0, 0]),
        Err(SurfaceMotionError::InvalidPolygon)
    );
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("movement does not dispatch strategies")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("movement does not dispatch death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("movement does not resume maps")
    }
}

#[test]
fn unknown_geometry_preserves_mutated_prefix_and_scene_latches_the_failure() {
    let mut objects = ObjectStore::new();
    let owner = actor(&mut objects);
    let collider = actor(&mut objects);
    objects.get_mut(collider).unwrap().base.shape = ShapeId::from_catalog_index(u16::MAX);
    objects
        .get_mut(collider)
        .unwrap()
        .base
        .contacts
        .first_strategy_visit = false;
    objects.get_mut(owner).unwrap().base.velocity = Vector3 { x: 7, y: 4, z: 5 };
    let mut world = ScenePathWorld::new(RandomState::default());
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut scene = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        callbacks: &mut callbacks,
        catalog: &catalog,
        statement_budget: 1,
    };
    assert_eq!(
        scene.advance_surface_motion(owner, inputs()),
        Err(SceneError::SurfaceMotion(SurfaceMotionError::Query(
            SurfaceQueryError::UnknownShape {
                object: collider,
                shape: ShapeId::from_catalog_index(u16::MAX)
            }
        )))
    );
    assert_eq!(
        scene.objects.get(owner).unwrap().base.position,
        Vector3 { x: 7, y: 4, z: 5 }
    );
    assert_eq!(
        scene.objects.get(owner).unwrap().base.velocity,
        Vector3 {
            x: 56,
            y: 32,
            z: 40
        }
    );
    scene.objects.get_mut(collider).unwrap().base.shape = ShapeId::EMPTY;
    assert_eq!(
        scene.advance_surface_motion(owner, inputs()),
        Err(SceneError::Faulted)
    );
    assert_eq!(
        scene.objects.get(owner).unwrap().base.position,
        Vector3 { x: 7, y: 4, z: 5 }
    );
}
