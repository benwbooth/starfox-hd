use super::*;
use crate::collision_surface::SurfaceMode;
use crate::path_program::PathCatalog;
use crate::player_storage::PlayerStorageInputs;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::view_transition::ViewTransitionMode;
use crate::weapon_dispatch::WeaponState;
use crate::{ObjectSpawnDefaults, RandomState};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected external strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
    owner: ObjectId,
    proxy: ObjectId,
}
impl Scene {
    fn new(material: u8) -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let proxy = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        objects.get_mut(owner).unwrap().base.hit_points = 30;
        objects.get_mut(proxy).unwrap().base.hit_points = 1;
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut execution = SceneExecution::default();
        player_storage::replace(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        world.primary_player = Some(owner);
        world.surface_mode = Some(SurfaceMode { flags: 1 });
        world.player_carry_mode = Some(0);
        world.scene.player_configuration = Some(0);
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 42,
            run_when_paused: false,
        });
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world
            .player_mut(&objects, owner)
            .unwrap()
            .surface
            .as_mut()
            .unwrap()
            .material = material;
        Self {
            objects,
            world,
            execution,
            owner,
            proxy,
            catalog: authored_paths::catalog(),
            callbacks: Callbacks,
        }
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 128,
        }
    }
    fn respond(&mut self, number: Option<u8>) -> Result<SurfaceResponse, SceneError<&'static str>> {
        let owner = self.owner;
        self.host().respond_player_surface(
            owner,
            SurfaceContext {
                wing_child_number: number,
                alternate_thrust_target: Some(-19),
            },
        )
    }
    fn epoch(&mut self, schedule: &mut StrategySchedule) {
        self.execution.positional.begin_epoch();
        self.host().begin_strategy_epoch(schedule).unwrap();
        schedule
            .run_overlapping(&mut self.host(), || false)
            .unwrap();
        schedule.run_remainder(&mut self.host()).unwrap();
        self.host().clean_epoch().unwrap();
    }
}

#[test]
fn zero_surface_mode_clears_clipping_before_skipping_all_player_reads_but_still_checks_sound_mode()
{
    let mut scene = Scene::new(4);
    scene.world.surface_mode = Some(SurfaceMode { flags: 0xF8 });
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .extension
        .clipping_plane = ClippingPlaneSelection::SECOND;
    scene.world.release_player_bindings(scene.owner);
    scene.world.weapons = None;
    scene.world.player_carry_mode = None;
    let response = scene.respond(None).unwrap();
    assert_eq!(
        response,
        SurfaceResponse {
            contact: false,
            effect_events: 0,
            alternate_thrust_target: Some(-19)
        }
    );
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .clipping_plane,
        ClippingPlaneSelection::DISABLED
    );
    assert_eq!(scene.objects.len(), 2);
    scene.world.view_transition_mode = None;
    scene.world.spawn_defaults = None;
    assert_eq!(
        scene.respond(None),
        Err(SceneError::PlayerSurface(SurfaceError::MissingViewMode))
    );
    assert!(scene.execution.is_faulted());
}

#[test]
fn lower_contact_carry_gate_suppresses_effects_and_recoil_but_not_fine_pitch_or_low_bank_damping() {
    let mut scene = Scene::new(4);
    scene.world.player_carry_mode = Some(1);
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .carry_selected_player = true;
    let records = scene.world.player_mut(&scene.objects, scene.owner).unwrap();
    records.pose.as_mut().unwrap().shoulder_bank = 0x12FD;
    records
        .vertical
        .as_mut()
        .unwrap()
        .profile
        .upper_height_offset = -257;
    player_storage::get_mut(
        &scene.objects,
        &mut scene.execution.paths.runtime.resources,
        scene.owner,
    )
    .unwrap()
    .fine_pitch = 0x405A;
    let response = scene.respond(None).unwrap();
    assert_eq!(
        response,
        SurfaceResponse {
            contact: false,
            effect_events: 0,
            alternate_thrust_target: Some(-2)
        }
    );
    let records = scene.world.player(&scene.objects, scene.owner).unwrap();
    assert_eq!(records.pose.unwrap().shoulder_bank, 0x12FF);
    assert_eq!(records.contact.unwrap().hit.camera_pitch_recoil, 0);
    assert_eq!(
        player_storage::get(
            &scene.objects,
            &scene.execution.paths.runtime.resources,
            scene.owner
        )
        .unwrap()
        .fine_pitch,
        0xE85A
    );
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .clipping_plane,
        ClippingPlaneSelection::SECOND
    );
    assert_eq!(
        scene.objects.get(scene.proxy).unwrap().base.position,
        // Zero angles still pass through the original quantized cosine
        // multiplies; they are not an exact identity transform.
        Vector3 { x: -33, y: 3, z: 0 }
    );
    assert_eq!(scene.objects.len(), 2);
}

#[test]
fn upper_contact_installs_wings_even_when_carry_mode_suppresses_recoil_and_return_value() {
    let mut scene = Scene::new(4);
    scene.world.scene.player_configuration = Some(9);
    scene.world.player_carry_mode = Some(1);
    scene
        .objects
        .get_mut(scene.owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .carry_selected_player = true;
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .vertical
        .as_mut()
        .unwrap()
        .profile
        .upper_height_offset = -100;
    let response = scene.respond(Some(91)).unwrap();
    assert!(!response.contact);
    assert_eq!(response.effect_events, 3);
    assert_eq!(scene.objects.len(), 4);
    for (_, actor) in scene
        .objects
        .active_objects()
        .filter(|(_, actor)| actor.base.attachment.is_some())
    {
        assert_eq!(actor.base.child_number, 91);
        assert_eq!(actor.base.path, Some(authored_paths::ALTERNATE_EXHAUST));
    }
    assert_eq!(
        scene
            .objects
            .get(scene.owner)
            .unwrap()
            .extension
            .clipping_plane,
        ClippingPlaneSelection::DISABLED
    );
}

#[test]
fn missing_inherited_wing_number_preserves_fresh_unassigned_allocation_then_latches_against_retry()
{
    let mut scene = Scene::new(0);
    assert_eq!(
        scene.respond(None),
        Err(SceneError::PlayerSurface(
            SurfaceError::MissingWingChildNumber
        ))
    );
    assert_eq!(scene.objects.len(), 3);
    let before = scene.objects.clone();
    assert_eq!(scene.respond(Some(4)), Err(SceneError::Faulted));
    assert_eq!(scene.objects, before);
    assert_eq!(
        scene.objects.get(scene.owner).unwrap().base.first_child,
        None
    );
    assert!(scene
        .objects
        .active_objects()
        .all(|(_, actor)| actor.base.behavior == Behavior::Unassigned));
}

#[test]
fn full_surface_visit_installs_effects_then_real_scheduler_runs_and_retires_both_families() {
    for (material, lifetime) in [(0, 3), (4, 4), (1, 5)] {
        let mut scene = Scene::new(material);
        let response = scene.respond(Some(73)).unwrap();
        assert!(response.contact);
        assert_eq!(response.effect_events, if material == 0 { 3 } else { 7 });
        assert_eq!(scene.objects.len(), 4);
        let mut schedule = StrategySchedule::default();
        // Source surface sound skips in scripted views; the created effects
        // retain their independently installed run-during-pause exemption.
        scene.world.view_transition_mode = Some(ViewTransitionMode { flags: 2 });
        for visit in 1..=lifetime {
            scene.epoch(&mut schedule);
            assert_eq!(scene.objects.len(), if visit == lifetime { 2 } else { 4 });
        }
        assert!(scene
            .objects
            .get(scene.owner)
            .unwrap()
            .base
            .first_child
            .is_none());
    }
}
