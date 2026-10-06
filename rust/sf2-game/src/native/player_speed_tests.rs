use super::*;
use crate::path_program::{ActionGate, PathCatalog};
use crate::path_runtime::PathRuntime;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Buttons, InputState, Object, ObjectKind, RandomState, ShapeId};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    owner: ObjectId,
    _runtime: PathRuntime,
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
        world.action_gate = Some(ActionGate { code: 0 });
        world.processed_player_input = Some(InputState::default());
        world.scene.player_configuration = Some(0);
        let mut fixture = Self {
            objects,
            world,
            owner,
            _runtime: runtime,
        };
        fixture.record().auxiliary.as_mut().unwrap().mode = 1;
        fixture
    }
    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn run(&mut self, context: SpeedContext) -> Result<(), SpeedError> {
        advance(&mut self.objects, &mut self.world, self.owner, context)
    }
}

#[test]
fn signed_thrust_uses_wide_difference_and_does_not_touch_linked_charge_flags() {
    let mut fixture = Fixture::new();
    fixture.record().auxiliary.as_mut().unwrap().action_flags = HEADING_LOCKED;
    fixture.record().charge.as_mut().unwrap().linked_mode = true;
    fixture
        .record()
        .charge
        .as_mut()
        .unwrap()
        .linked_muzzle_disabled = true;
    for current in i8::MIN..=i8::MAX {
        for target in i8::MIN..=i8::MAX {
            fixture.record().speed.as_mut().unwrap().thrust = current;
            let charge = fixture.record().charge;
            fixture
                .run(SpeedContext {
                    thrust_target: Some(target),
                    alternate_thrust_target: None,
                })
                .unwrap();
            let delta = i16::from(target) - i16::from(current);
            let step = if delta == 0 {
                0
            } else {
                delta.signum() * (delta.abs() / 8).max(1)
            };
            assert_eq!(
                fixture.record().speed.unwrap().thrust,
                (i16::from(current) + step) as i8
            );
            assert_eq!(fixture.record().charge, charge);
        }
    }
}

#[test]
fn forced_gates_skip_later_dependencies_and_write_base_before_missing_thrust() {
    let mut fixture = Fixture::new();
    fixture.world.action_gate = Some(ActionGate { code: 255 });
    fixture.record().contact = None;
    fixture.record().visit = None;
    fixture.record().auxiliary = None;
    fixture.world.processed_player_input = None;
    fixture.world.scene.player_configuration = None;
    fixture.record().speed.as_mut().unwrap().thrust = -128;
    fixture.run(SpeedContext::default()).unwrap();
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 4);
    assert_eq!(fixture.record().speed.unwrap().thrust, -112);
    fixture.record().speed = None;
    assert_eq!(
        fixture.run(SpeedContext::default()),
        Err(SpeedError::MissingSpeed(fixture.owner))
    );
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 8);
    fixture.world.action_gate = None;
    assert_eq!(
        fixture.run(SpeedContext::default()),
        Err(SpeedError::World(WorldInputError::MissingActionGate))
    );
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 8);
    let mut fixture = Fixture::new();
    fixture.record().contact.as_mut().unwrap().ignores_contacts = true;
    fixture.record().visit = None;
    fixture.record().auxiliary = None;
    fixture.objects.get_mut(fixture.owner).unwrap().base.speed = 80;
    fixture.run(SpeedContext::default()).unwrap();
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 70);
}

#[test]
fn directional_turns_are_not_shoulders_and_even_visits_retain_surface_target() {
    let mut fixture = Fixture::new();
    fixture.world.scene.player_configuration = None;
    for button in [Button::Left, Button::Right] {
        fixture.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(button as u16),
            pressed: Buttons::default(),
        });
        fixture.objects.get_mut(fixture.owner).unwrap().base.speed = 100;
        fixture.record().speed.as_mut().unwrap().thrust = 0;
        fixture.world.strategy_clock = 0;
        fixture
            .run(
                SurfaceResponse {
                    contact: true,
                    effect_events: 6,
                    alternate_thrust_target: None,
                }
                .into(),
            )
            .unwrap();
        assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 100);
        assert_eq!(fixture.record().speed.unwrap().thrust, 1);
        fixture.world.strategy_clock = 1;
        fixture.run(SpeedContext::default()).unwrap();
        assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 99);
        assert_eq!(fixture.record().speed.unwrap().thrust, 0);
    }
    fixture.world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(Button::LeftShoulder as u16 | Button::RightShoulder as u16),
        pressed: Buttons::default(),
    });
    fixture.objects.get_mut(fixture.owner).unwrap().base.speed = 100;
    fixture.run(SpeedContext::default()).unwrap();
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 93);
}

#[test]
fn boost_precedes_brake_and_locked_targets_are_only_required_on_consuming_branches() {
    let mut fixture = Fixture::new();
    fixture.record().auxiliary.as_mut().unwrap().action_flags = BOOST | BRAKE | HEADING_LOCKED;
    fixture.world.processed_player_input = None;
    fixture.run(SpeedContext::default()).unwrap();
    assert_eq!(fixture.record().speed.unwrap().thrust, 10);
    fixture.record().auxiliary.as_mut().unwrap().action_flags = BRAKE | HEADING_LOCKED;
    fixture.world.scene.player_configuration = None;
    fixture.run(SpeedContext::default()).unwrap();
    assert_eq!(fixture.record().speed.unwrap().thrust, 5);
    fixture.record().auxiliary.as_mut().unwrap().action_flags = HEADING_LOCKED;
    fixture.objects.get_mut(fixture.owner).unwrap().base.speed = 0;
    assert_eq!(
        fixture.run(SpeedContext::default()),
        Err(SpeedError::MissingThrustTarget)
    );
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 5);
    fixture.record().auxiliary.as_mut().unwrap().mode = 0;
    assert_eq!(
        fixture.run(SpeedContext::default()),
        Err(SpeedError::MissingAlternateThrustTarget)
    );
    assert_eq!(fixture.objects.get(fixture.owner).unwrap().base.speed, 5);
    fixture
        .run(SpeedContext {
            thrust_target: None,
            alternate_thrust_target: Some(-127),
        })
        .unwrap();
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("speed does not dispatch actors")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("speed does not dispatch death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("speed does not resume maps")
    }
}

#[test]
fn scene_fault_latches_partial_base_speed_before_retry() {
    let mut fixture = Fixture::new();
    fixture.record().speed = None;
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut execution = SceneExecution::default();
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
        scene.advance_player_speed(fixture.owner, SpeedContext::default()),
        Err(SceneError::PlayerSpeed(SpeedError::MissingSpeed(
            fixture.owner
        )))
    );
    assert_eq!(scene.objects.get(fixture.owner).unwrap().base.speed, 5);
    scene
        .world
        .player_mut(scene.objects, fixture.owner)
        .unwrap()
        .speed = Some(Default::default());
    assert_eq!(
        scene.advance_player_speed(fixture.owner, SpeedContext::default()),
        Err(SceneError::Faulted)
    );
    assert_eq!(scene.objects.get(fixture.owner).unwrap().base.speed, 5);
}
