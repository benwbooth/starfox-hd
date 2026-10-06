use super::*;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_storage::PlayerStorageInputs;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Buttons, InputState, Object, ObjectKind, RandomState, ShapeId};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
    runtime: PathRuntime,
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
                reserve_shield: 90,
                score: Default::default(),
            },
        )
        .unwrap();
        world.processed_player_input = Some(Default::default());
        world.player_pitch_target = Some(0);
        Self {
            objects,
            world,
            runtime,
            owner,
        }
    }
    fn record(&mut self) -> &mut PlayerVerticalControl {
        control(&self.objects, &mut self.world, self.owner).unwrap()
    }
    fn input(&mut self, held: u16, pressed: u16) {
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(pressed),
        });
    }
}

#[test]
fn allocation_clears_vertical_state_and_configuration_changes_only_profile() {
    let mut f = Fixture::new();
    assert_eq!(*f.record(), PlayerVerticalControl::default());
    f.record().latched_input = 12345;
    f.record().limit_flags = 255;
    let before = *f.world.player(&f.objects, f.owner).unwrap();
    let profile = VerticalProfile {
        upper_height_offset: 600,
        lower_height_offset: 12,
        up_pitch: 15,
        down_pitch: 241,
    };
    configure(&f.objects, &mut f.world, f.owner, profile).unwrap();
    let mut expected = before;
    expected.vertical.as_mut().unwrap().profile = profile;
    assert_eq!(*f.world.player(&f.objects, f.owner).unwrap(), expected);
}

#[test]
fn retained_history_tracks_all_bits_and_does_not_relatch_a_cleared_held_bit() {
    let mut f = Fixture::new();
    for held in 0..=u16::MAX {
        f.record().previous_held = 0;
        f.record().latched_input = 0;
        f.input(held, 0);
        retain_input(&f.objects, &mut f.world, f.owner).unwrap();
        assert_eq!(f.record().latched_input, held);
        f.record().latched_input = held & !(Button::Down as u16);
        retain_input(&f.objects, &mut f.world, f.owner).unwrap();
        assert_eq!(f.record().latched_input, held & !(Button::Down as u16));
        f.input(0, held);
        retain_input(&f.objects, &mut f.world, f.owner).unwrap();
        assert_eq!(f.record().latched_input, 0);
        assert_eq!(f.record().previous_held, 0);
    }
}

#[test]
fn flight_uses_retained_down_but_held_mode_uses_held_down_and_retains_fine_neutral_pitch() {
    let mut f = Fixture::new();
    f.record().profile = VerticalProfile {
        up_pitch: 15,
        down_pitch: 241,
        ..Default::default()
    };
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .action_flags = ACTIVE;
    for held in [0, 0x400, 0x800, 0xC00] {
        for latched in [0, 0x400] {
            for flags in 0..=u8::MAX {
                f.input(held, 0xC00);
                f.record().latched_input = latched;
                f.record().control_flags = flags;
                flight_pitch(&f.objects, &mut f.world, f.owner).unwrap();
                let expected = if held & 0x800 != 0 {
                    0xF00
                } else if latched != 0 {
                    0xF100
                } else {
                    0
                };
                assert_eq!(f.world.player_pitch_target, Some(expected));
                assert_eq!(f.record().control_flags, flags & !PITCH_CONTROL);
                player_storage::get_mut(&f.objects, &mut f.runtime.resources, f.owner)
                    .unwrap()
                    .fine_pitch = 12345;
                held_pitch(&f.objects, &mut f.world, &f.runtime.resources, f.owner).unwrap();
                let expected = if held & 0x800 != 0 {
                    0xF00
                } else if held & 0x400 != 0 {
                    0xF100
                } else {
                    12345
                };
                assert_eq!(f.world.player_pitch_target, Some(expected));
                assert_eq!(f.record().control_flags, flags | PITCH_CONTROL);
            }
        }
    }
}

#[test]
fn missing_inputs_preserve_completed_writes_and_inactive_held_pitch_needs_no_control() {
    let mut f = Fixture::new();
    f.record().pitch_adjustment = 99;
    f.world.processed_player_input = None;
    assert_eq!(
        flight_pitch(&f.objects, &mut f.world, f.owner),
        Err(VerticalError::MissingProcessedInput)
    );
    assert_eq!(f.record().pitch_adjustment, 0);
    assert_eq!(f.world.player_pitch_target, Some(0));
    f.input(0x800, 0);
    f.record().profile.up_pitch = 15;
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    assert_eq!(
        flight_pitch(&f.objects, &mut f.world, f.owner),
        Err(VerticalError::MissingEnvironmentPlane)
    );
    assert_eq!(f.world.player_pitch_target, Some(3840));
    f.world.player_mut(&f.objects, f.owner).unwrap().vertical = None;
    f.world.player_pitch_target = None;
    f.world.processed_player_input = None;
    held_pitch(&f.objects, &mut f.world, &f.runtime.resources, f.owner).unwrap();
    assert_eq!(f.world.player_pitch_target, Some(0));
}

#[test]
fn plane_override_compares_wrapped_difference_not_signed_magnitudes() {
    let mut f = Fixture::new();
    f.world
        .player_mut(&f.objects, f.owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    for plane in [i16::MIN, -1, 0, 50, i16::MAX] {
        f.world.environment_plane_height = Some(plane);
        for height in 0..=u16::MAX {
            f.objects.get_mut(f.owner).unwrap().base.position.y = height as i16;
            flight_pitch(&f.objects, &mut f.world, f.owner).unwrap();
            let expected = if (height.wrapping_sub(plane.wrapping_sub(50) as u16) & 0x8000) == 0 {
                49152
            } else {
                0
            };
            assert_eq!(f.world.player_pitch_target, Some(expected));
        }
    }
}

#[test]
fn hard_lower_limit_includes_zero_and_neutral_recovery_runs_after_limit_lean() {
    let mut f = Fixture::new();
    f.world.strategy_clock = 1;
    f.record().limit_flags = 255;
    hard_limits(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(f.record().limit_flags, 251);
    assert_eq!(f.record().motion_axes, 0xA0);
    assert_eq!(*lean(&f.objects, &mut f.world, f.owner).unwrap(), 0);
    f.input(0x400, 0);
    hard_limits(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(*lean(&f.objects, &mut f.world, f.owner).unwrap(), 1);
    f.record().profile = VerticalProfile {
        upper_height_offset: -1,
        lower_height_offset: -1,
        ..Default::default()
    };
    f.world.player_pitch_target = Some((-3i16) as u16);
    hard_limits(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(f.world.player_pitch_target, Some((-2i16) as u16));
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not map")
    }
}

#[test]
fn scene_config_selects_primary_and_partial_vertical_failure_is_not_replayed() {
    let mut f = Fixture::new();
    let other = f
        .objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    player_storage::initialize(
        &mut f.objects,
        &mut f.world,
        &mut f.runtime,
        other,
        PlayerStorageInputs {
            pilot_code: 1,
            reserve_shield: 90,
            score: Default::default(),
        },
    )
    .unwrap();
    f.world.primary_player = Some(other);
    let mut execution = SceneExecution::default();
    execution.paths.runtime = f.runtime;
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut callbacks = Callbacks;
    let mut actors = SceneActors {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 100,
    };
    let profile = VerticalProfile {
        up_pitch: 19,
        ..Default::default()
    };
    actors.configure_player_vertical(profile).unwrap();
    assert_eq!(
        actors
            .world
            .player(actors.objects, other)
            .unwrap()
            .vertical
            .unwrap()
            .profile,
        profile
    );
    assert_eq!(
        actors
            .world
            .player(actors.objects, f.owner)
            .unwrap()
            .vertical
            .unwrap()
            .profile,
        VerticalProfile::default()
    );
    actors.world.processed_player_input = Some(InputState {
        held: Buttons::from_bits(0x800),
        pressed: Buttons::from_bits(0),
    });
    actors.retain_player_input(other).unwrap();
    actors.world.player_mut(actors.objects, other).unwrap().pose = None;
    assert_eq!(
        actors.advance_player_vertical(other, VerticalMode::Flight),
        Err(SceneError::PlayerVertical(VerticalError::MissingPose(
            other
        )))
    );
    let before = *actors.world.player(actors.objects, other).unwrap();
    let target = actors.world.player_pitch_target;
    assert_eq!(
        actors.advance_player_vertical(other, VerticalMode::Flight),
        Err(SceneError::Faulted)
    );
    assert_eq!(*actors.world.player(actors.objects, other).unwrap(), before);
    assert_eq!(actors.world.player_pitch_target, target);
}
