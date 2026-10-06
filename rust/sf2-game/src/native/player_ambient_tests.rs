use super::*;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    _runtime: PathRuntime,
    owner: ObjectId,
}

impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 0,
                score: Default::default(),
            },
        )
        .unwrap();
        world.view_transition_mode = Some(ViewTransitionMode { flags: 0 });
        Self {
            objects,
            world,
            _runtime: runtime,
            owner,
        }
    }
    fn run(&mut self) -> Result<(), AmbientError> {
        advance(&self.objects, &mut self.world, self.owner)
    }
    fn record(&mut self) -> &mut crate::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
}

#[test]
fn phases_wrap_before_range_checks_and_only_special_mode_holds_bank() {
    let mut fixture = Fixture::new();
    for phase in 0..=u8::MAX {
        for mode in 0..=u8::MAX {
            let initial = PlayerAmbient {
                bank_phase: phase,
                offset_phase: phase,
                retained_offset: if phase & 1 == 0 { i16::MAX } else { i16::MIN },
            };
            fixture.record().ambient = Some(initial);
            fixture.record().auxiliary.as_mut().unwrap().mode = mode;
            fixture.record().pose.as_mut().unwrap().ambient_bank = -127;
            fixture.run().unwrap();
            let actual = fixture.record().ambient.unwrap();
            let next = phase.wrapping_add(1);
            let bank = if next < 30 { next } else { 0 };
            let offset = if next < 32 { next } else { 0 };
            assert_eq!(
                actual.bank_phase,
                if mode & 0xF0 == 0x30 { phase } else { bank }
            );
            assert_eq!(
                fixture.record().pose.unwrap().ambient_bank,
                if mode & 0xF0 == 0x30 {
                    -127
                } else {
                    BANK_WAVE[usize::from(bank)]
                }
            );
            assert_eq!(actual.offset_phase, offset);
            assert_eq!(
                actual.retained_offset,
                initial
                    .retained_offset
                    .wrapping_add(i16::from(OFFSET_WAVE[usize::from(offset)]))
            );
        }
    }
}

#[test]
fn scripted_view_skips_before_player_lookup_and_special_mode_needs_no_pose() {
    let mut fixture = Fixture::new();
    fixture.world.view_transition_mode = Some(ViewTransitionMode { flags: 2 });
    fixture.objects.remove(fixture.owner).unwrap();
    fixture.run().unwrap();
    fixture.world.view_transition_mode = None;
    assert_eq!(fixture.run(), Err(AmbientError::MissingViewMode));
    let mut fixture = Fixture::new();
    fixture.record().pose = None;
    fixture.record().auxiliary.as_mut().unwrap().mode = 0x3F;
    fixture.run().unwrap();
    assert_eq!(fixture.record().ambient.unwrap().bank_phase, 0);
    assert_eq!(fixture.record().ambient.unwrap().offset_phase, 1);
    fixture.record().auxiliary.as_mut().unwrap().mode = 0x10;
    assert_eq!(fixture.run(), Err(AmbientError::MissingPose(fixture.owner)));
    assert_eq!(fixture.record().ambient.unwrap().bank_phase, 1);
    assert_eq!(fixture.record().ambient.unwrap().offset_phase, 1);
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("ambient motion never dispatches strategies")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("ambient motion never dispatches death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("ambient motion never resumes maps")
    }
}

#[test]
fn scene_fault_latches_before_repeating_partial_phase_update() {
    let mut fixture = Fixture::new();
    fixture.record().pose = None;
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors {
        objects: &mut fixture.objects,
        world: &mut fixture.world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 1,
    };
    assert_eq!(
        scene.advance_player_ambient(fixture.owner),
        Err(SceneError::PlayerAmbient(AmbientError::MissingPose(
            fixture.owner
        )))
    );
    let record = scene
        .world
        .player_mut(scene.objects, fixture.owner)
        .unwrap();
    assert_eq!(record.ambient.unwrap().bank_phase, 1);
    record.pose = Some(Default::default());
    assert_eq!(
        scene.advance_player_ambient(fixture.owner),
        Err(SceneError::Faulted)
    );
    assert_eq!(
        scene
            .world
            .player(scene.objects, fixture.owner)
            .unwrap()
            .ambient
            .unwrap()
            .bank_phase,
        1
    );
}
