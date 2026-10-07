use super::*;
use crate::collision_surface::SurfaceMode;
use crate::path_program::PathCatalog;
use crate::player_motion::PlayerSurfaceSupport;
use crate::player_storage::{self, PlayerStorageInputs};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, Buttons, InputState, Object, ObjectKind, RandomState, ShapeId};

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
    catalog: PathCatalog,
    callbacks: Callbacks,
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
        let proxy = allocate();
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut execution = SceneExecution::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 32,
                score: Default::default(),
            },
        )
        .unwrap();
        objects.get_mut(owner).unwrap().base.position.y = -100;
        world.primary_player = Some(owner);
        world.action_gate = Some(Default::default());
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        world.player_pitch_target = Some(0x1234);
        world.player_yaw_increment = Some(0x2345);
        world.player_roll_increment = Some(0x3456);
        world.processed_player_input = Some(Default::default());
        world.contacts_enabled = Some(true);
        world.view_transition_mode = Some(ViewTransitionMode::default());
        world.surface_mode = Some(SurfaceMode { flags: 0 });
        world.player_carry_mode = Some(0);
        world.environment_plane_height = Some(0);
        world.player_surface_support = Some(PlayerSurfaceSupport::default());
        world.scene.player_configuration = Some(0);
        world.reflect_all_contacts = Some(false);
        world.spawn_defaults = Some(Default::default());
        world
            .player_mut(&objects, owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = 0x11;
        Self {
            objects,
            world,
            execution,
            owner,
            catalog: PathCatalog::new(vec![]).unwrap(),
            callbacks: Callbacks,
        }
    }
    fn records(&mut self) -> &mut crate::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn advance(&mut self) -> Result<FlightResult, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 64,
        }
        .advance_player_retained_pitch_flight(self.owner, FlightModeContext::default())
    }
}

#[test]
fn missing_input_stops_before_history_and_steering_reset() {
    let mut f = Fixture::new();
    f.world.processed_player_input = None;
    f.records().vertical.as_mut().unwrap().previous_held = 0xFFFF;
    f.records().steering.as_mut().unwrap().camera_bank_target = 6;
    assert_eq!(
        f.advance(),
        Err(SceneError::PlayerFlightMode(FlightModeError::Vertical(
            VerticalError::MissingProcessedInput
        )))
    );
    assert_eq!(f.records().vertical.unwrap().previous_held, 0xFFFF);
    assert_eq!(f.records().steering.unwrap().camera_bank_target, 6);
    assert_eq!(f.world.player_pitch_target, Some(0x1234));
    assert_eq!(f.world.player_yaw_increment, Some(0x2345));
    assert_eq!(f.world.player_roll_increment, Some(0x3456));
}

#[test]
fn incomplete_steering_keeps_history_prefix_without_replaying_on_scene_retry() {
    let mut f = Fixture::new();
    f.records().roll.as_mut().unwrap().impulse = 32;
    f.records().steering = None;
    f.world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(Button::Left as u16),
        pressed: Default::default(),
    });
    assert_eq!(
        f.advance(),
        Err(SceneError::PlayerFlightMode(FlightModeError::Steering(
            SteeringError::MissingSteering(f.owner)
        )))
    );
    assert_eq!(
        f.records().vertical.unwrap().previous_held,
        Button::Left as u16
    );
    assert_eq!(
        f.records().vertical.unwrap().latched_input,
        Button::Left as u16
    );
    assert_eq!(f.records().roll.unwrap().impulse, 32);
    assert_eq!(f.world.player_pitch_target, Some(0x1234));
    assert_eq!(f.advance(), Err(SceneError::Faulted));
    assert_eq!(f.records().roll.unwrap().impulse, 32);
}

#[test]
fn real_same_visit_mode_clears_stale_targets_and_runs_flight_once() {
    let mut f = Fixture::new();
    f.records().roll.as_mut().unwrap().impulse = 32;
    f.records().steering.as_mut().unwrap().camera_bank_target = 6;
    f.advance().unwrap();
    assert_eq!(f.world.player_pitch_target, Some(0));
    assert_eq!(f.world.player_yaw_increment, Some(0));
    assert_eq!(f.world.player_roll_increment, Some(0));
    assert_eq!(f.records().steering.unwrap().camera_bank_target, 0);
    assert_eq!(f.records().roll.unwrap().impulse, 30);
    assert_eq!(f.records().ambient.unwrap().bank_phase, 1);
    assert_eq!(f.records().vertical.unwrap().motion_axes, 0xE0);
    assert_ne!(f.objects.get(f.owner).unwrap().base.position.z, 0);
}

#[test]
fn select_cue_uses_original_unsided_channel_even_for_secondary_player() {
    let mut f = Fixture::new();
    f.world.primary_player = None;
    f.world.processed_player_input = Some(InputState {
        held: Default::default(),
        pressed: Buttons::from_bits(Button::Select as u16),
    });
    f.advance().unwrap();
    let events: Vec<_> = f.world.audio.take_events().into_iter().flatten().collect();
    assert_eq!(
        events.last(),
        Some(&SoundEvent::Authored(AuthoredCue::new(
            54,
            0,
            PlayerTarget::Primary
        )))
    );
    f.world.processed_player_input = Some(Default::default());
    f.advance().unwrap();
    assert!(
        f.world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .all(|event| event
                != SoundEvent::Authored(AuthoredCue::new(54, 0, PlayerTarget::Primary)))
    );
}

#[test]
fn a_later_flight_failure_retains_control_prefix_and_does_not_advance_roll_twice() {
    let mut f = Fixture::new();
    f.world.contacts_enabled = None;
    f.records().roll.as_mut().unwrap().impulse = 32;
    assert_eq!(
        f.advance(),
        Err(SceneError::PlayerFlightMode(FlightModeError::Flight(
            FlightError::Throttle(crate::player_throttle::ThrottleError::MissingActivity)
        )))
    );
    assert_eq!(f.world.player_yaw_increment, Some(0));
    assert_eq!(f.records().roll.unwrap().impulse, 30);
    assert_eq!(f.records().ambient.unwrap().bank_phase, 1);
    assert_eq!(f.advance(), Err(SceneError::Faulted));
    assert_eq!(f.records().roll.unwrap().impulse, 30);
    assert_eq!(f.records().ambient.unwrap().bank_phase, 1);
}
