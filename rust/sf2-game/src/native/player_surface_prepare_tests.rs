use super::*;
use crate::path_program::PathCatalog;
use crate::player_motion::PlayerMotion;
use crate::player_surface::PlayerSurface;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
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
    support: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Player,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let owner = allocate();
        let support = allocate();
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    motion: Some(PlayerMotion::default()),
                    surface: Some(PlayerSurface {
                        material: 77,
                        plane_height: -123,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        world.surface_clipping_plane_height = Some(999);
        Self {
            objects,
            world,
            execution: Default::default(),
            owner,
            support,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn prepare(&mut self) -> Result<Option<i16>, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .prepare_player_surface(self.owner)
    }
    fn contact(&mut self, flags: u8) {
        let contact = &mut self
            .objects
            .get_mut(self.owner)
            .unwrap()
            .extension
            .surface_contact;
        contact.supporting_object = Some(self.support);
        contact.flags = flags;
    }
}

#[test]
fn missing_support_uses_environment_and_does_not_require_motion_or_contact_material() {
    let mut f = Fixture::new();
    f.records().motion = None;
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .surface_contact
        .flags = 1;
    f.world.player_carry_mode = Some(197);
    f.world.environment_plane_height = Some(-17);
    assert_eq!(f.prepare(), Ok(Some(-17)));
    assert_eq!(f.records().surface.unwrap().material, 197);
    assert_eq!(f.records().surface.unwrap().plane_height, -17);
    assert_eq!(f.world.surface_clipping_plane_height, Some(-17));
    f.world.environment_plane_height = Some(0);
    assert_eq!(f.prepare(), Ok(None));
    assert_eq!(f.records().surface.unwrap().plane_height, 0);
    assert_eq!(f.world.surface_clipping_plane_height, Some(-17));
}

#[test]
fn nonplane_material_never_dereferences_stale_support_or_environment() {
    let mut f = Fixture::new();
    f.contact(3);
    f.objects.remove(f.support).unwrap();
    f.records().motion = None;
    assert_eq!(f.prepare(), Ok(None));
    assert_eq!(f.records().surface.unwrap().material, 3);
    assert_eq!(f.records().surface.unwrap().plane_height, 0);
    assert_eq!(f.world.surface_clipping_plane_height, Some(999));
}

#[test]
fn ordinary_contact_uses_wrapped_retained_height_not_actor_position() {
    let mut f = Fixture::new();
    f.contact(0);
    f.objects.remove(f.support).unwrap();
    f.objects.get_mut(f.owner).unwrap().base.position.y = 123;
    f.records().motion.as_mut().unwrap().surface_height = i16::MAX;
    f.world.environment_plane_height = Some(-1);
    assert_eq!(f.prepare(), Ok(None));
    assert_eq!(f.records().surface.unwrap().material, 0);
    assert_eq!(f.world.surface_clipping_plane_height, Some(999));
}

#[test]
fn raised_material_publishes_wrapped_zero_as_a_real_selected_plane() {
    let mut f = Fixture::new();
    f.contact(4);
    f.records().motion = None;
    let support = f.objects.get_mut(f.support).unwrap();
    support.base.position.y = i16::MIN;
    support.extension.path_state.script_value = 0x8000;
    assert_eq!(f.prepare(), Ok(Some(0)));
    assert_eq!(f.records().surface.unwrap().material, 4);
    assert_eq!(f.records().surface.unwrap().plane_height, 0);
    assert_eq!(f.world.surface_clipping_plane_height, Some(0));
}

#[test]
fn failed_support_preserves_material_prefix_and_faults_scene_against_retry() {
    let mut f = Fixture::new();
    f.contact(1);
    f.objects.remove(f.support).unwrap();
    assert_eq!(
        f.prepare(),
        Err(SceneError::PlayerSurfacePreparation(
            SurfacePreparationError::World(WorldInputError::MissingActor(f.support))
        ))
    );
    assert_eq!(f.records().surface.unwrap().material, 1);
    assert_eq!(f.records().surface.unwrap().plane_height, -123);
    assert_eq!(f.world.surface_clipping_plane_height, Some(999));
    f.world.player_carry_mode = Some(6);
    f.world.environment_plane_height = Some(42);
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .surface_contact
        .supporting_object = None;
    assert_eq!(f.prepare(), Err(SceneError::Faulted));
    assert_eq!(f.records().surface.unwrap().material, 1);
}

#[test]
fn missing_environment_height_preserves_prior_material_write() {
    let mut f = Fixture::new();
    f.world.player_carry_mode = Some(5);
    assert_eq!(
        f.prepare(),
        Err(SceneError::PlayerSurfacePreparation(
            SurfacePreparationError::MissingEnvironmentPlane
        ))
    );
    assert_eq!(f.records().surface.unwrap().material, 5);
    assert_eq!(f.records().surface.unwrap().plane_height, -123);
    assert_eq!(f.world.surface_clipping_plane_height, Some(999));
}
