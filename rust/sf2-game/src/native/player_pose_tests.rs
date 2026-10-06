use super::*;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_roll;
use crate::player_storage::PlayerStorageInputs;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Button, Buttons, InputState, Object, ObjectKind, RandomState, ShapeId};

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
        world
            .player_mut(&objects, owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = 0x11;
        world.player_pitch_target = Some(0);
        world.player_yaw_increment = Some(0);
        Self {
            objects,
            world,
            runtime,
            owner,
        }
    }

    fn run(&mut self) -> Result<(), PoseError> {
        compose(
            &mut self.objects,
            &mut self.world,
            &mut self.runtime.resources,
            self.owner,
        )
    }

    fn storage(&mut self) -> &mut player_storage::PlayerStorage {
        player_storage::get_mut(&self.objects, &mut self.runtime.resources, self.owner).unwrap()
    }
}

#[test]
fn wrapped_quarter_chase_has_minimum_progress_at_both_numeric_widths() {
    for current in 0..=u8::MAX {
        for target in 0..=u8::MAX {
            let delta = target.wrapping_sub(current) as i8;
            let step = if delta == 0 {
                0
            } else if delta.abs_diff(0) < 4 {
                delta.signum()
            } else {
                delta / 4
            };
            assert_eq!(
                quarter_byte(current, target),
                current.wrapping_add(step as u8)
            );
        }
    }
    for current in [0, 1, 32767, 32768, 65535u16] {
        for target in 0..=u16::MAX {
            let delta = target.wrapping_sub(current) as i16;
            let step = if delta == 0 {
                0
            } else if delta.abs_diff(0) < 4 {
                delta.signum()
            } else {
                delta / 4
            };
            assert_eq!(
                quarter_word(current, target),
                current.wrapping_add(step as u16)
            );
        }
    }
}

#[test]
fn linked_pitch_rounds_down_while_bank_recovery_rounds_each_half_toward_zero() {
    let mut fixture = Fixture::new();
    for raw in 0..=u8::MAX {
        fixture.storage().fine_pitch = 0xFFFD;
        fixture.storage().bank = Angle::from_units(raw);
        let records = fixture
            .world
            .player_mut(&fixture.objects, fixture.owner)
            .unwrap();
        records.auxiliary.as_mut().unwrap().action_flags = 1;
        records.auxiliary.as_mut().unwrap().stored_rotation.pitch = Angle::from_units(raw);
        records.charge.as_mut().unwrap().linked_mode = true;
        fixture.run().unwrap();
        assert_eq!(fixture.storage().fine_pitch, 0xFFFE);
        let reversed = i16::from(raw.wrapping_neg() as i8);
        assert_eq!(
            fixture
                .objects
                .get(fixture.owner)
                .unwrap()
                .base
                .pitch
                .units(),
            (reversed + reversed.div_euclid(4)) as u8
        );
        let half = (raw as i8) / 2;
        assert_eq!(fixture.storage().bank.units(), (half + half / 2) as u8);
    }
}

#[test]
fn pose_uses_high_turning_lean_low_shoulder_bank_and_wrapping_yaw() {
    let mut fixture = Fixture::new();
    fixture.storage().fine_pitch = 0x7FFF;
    fixture.storage().fine_yaw = 0xFFFF;
    fixture.storage().bank = Angle::from_units(250);
    fixture.world.player_pitch_target = Some(0x8000);
    fixture.world.player_yaw_increment = Some(257);
    let records = fixture
        .world
        .player_mut(&fixture.objects, fixture.owner)
        .unwrap();
    records.pose = Some(PlayerPose {
        pitch_lean: 7,
        turning_lean: 0xFA99,
        yaw_trim: -3,
        ambient_bank: 9,
        steering_bank: -5,
        shoulder_bank: 0x01FE,
        heading_return_bank: 11,
        yaw_offset: 13,
    });
    records.roll.as_mut().unwrap().impulse = 32;
    let before = *records;
    fixture.run().unwrap();
    assert_eq!(fixture.storage().fine_pitch, 0x8000);
    assert_eq!(fixture.storage().fine_yaw, 256);
    assert_eq!(fixture.storage().bank.units(), 26);
    let base = &fixture.objects.get(fixture.owner).unwrap().base;
    assert_eq!(base.pitch.units(), 135);
    assert_eq!(base.yaw.units(), 6); // 1 + (-6) + (-2) + 13.
    assert_eq!(base.roll.units(), 39); // 26 + 9 - 2 - 5 + 11.
    let mut expected = before;
    expected.pose.as_mut().unwrap().yaw_trim = -2;
    expected.yaw_motion = Some(257);
    assert_eq!(
        *fixture
            .world
            .player(&fixture.objects, fixture.owner)
            .unwrap(),
        expected
    );
}

#[test]
fn heading_lock_and_inactive_roll_gates_do_not_require_unread_services() {
    for mode in [0x11, 0x20, 0x31] {
        let mut fixture = Fixture::new();
        fixture.storage().fine_yaw = 0x1234;
        fixture.storage().bank = Angle::from_units(99);
        fixture.objects.get_mut(fixture.owner).unwrap().base.roll = Angle::from_units(253);
        fixture.world.player_yaw_increment = None;
        let records = fixture
            .world
            .player_mut(&fixture.objects, fixture.owner)
            .unwrap();
        records.auxiliary.as_mut().unwrap().mode = mode;
        records.auxiliary.as_mut().unwrap().action_flags = HEADING_LOCKED;
        records.contact.as_mut().unwrap().ignores_contacts = mode == 0x11;
        records.pose.as_mut().unwrap().yaw_offset = 100;
        records.charge = None;
        records.roll = None;
        records.yaw_motion = None;
        fixture.run().unwrap();
        assert_eq!(fixture.storage().fine_yaw, 0x1234);
        assert_eq!(fixture.storage().bank.units(), 99);
        assert_eq!(
            fixture.objects.get(fixture.owner).unwrap().base.yaw.units(),
            18
        );
        assert_eq!(
            fixture
                .objects
                .get(fixture.owner)
                .unwrap()
                .base
                .roll
                .units(),
            255
        );
        assert_eq!(
            fixture
                .world
                .player(&fixture.objects, fixture.owner)
                .unwrap()
                .yaw_motion,
            None
        );
    }
}

#[test]
fn missing_later_inputs_preserve_exact_completed_pose_prefix() {
    for missing in 0..7 {
        let mut fixture = Fixture::new();
        fixture.storage().fine_pitch = 0x2000;
        fixture.storage().fine_yaw = 0x3000;
        fixture.world.player_yaw_increment = Some(512);
        fixture.world.player_pitch_target = Some(0x4000);
        let records = fixture
            .world
            .player_mut(&fixture.objects, fixture.owner)
            .unwrap();
        records.pose.as_mut().unwrap().yaw_trim = 64;
        records.auxiliary.as_mut().unwrap().action_flags = 1;
        let expected_error = match missing {
            0 => {
                records.pose = None;
                PoseError::MissingPose(fixture.owner)
            }
            1 => {
                records.auxiliary = None;
                PoseError::World(WorldInputError::MissingAuxiliary(fixture.owner))
            }
            2 => {
                fixture.world.player_pitch_target = None;
                PoseError::MissingPitchTarget
            }
            3 => {
                records.charge = None;
                PoseError::World(WorldInputError::MissingPlayerCharge(fixture.owner))
            }
            4 => {
                fixture.world.player_yaw_increment = None;
                PoseError::MissingYawIncrement
            }
            5 => {
                records.contact = None;
                PoseError::World(WorldInputError::MissingPlayerContact(fixture.owner))
            }
            _ => {
                records.roll = None;
                PoseError::MissingRoll(fixture.owner)
            }
        };
        assert_eq!(fixture.run(), Err(expected_error), "missing={missing}");
        assert_eq!(
            fixture.storage().fine_pitch,
            if missing < 3 { 0x2000 } else { 0x2800 }
        );
        assert_eq!(
            fixture.storage().fine_yaw,
            if missing < 5 { 0x3000 } else { 0x3200 }
        );
        let base = &fixture.objects.get(fixture.owner).unwrap().base;
        assert_eq!(base.pitch.units(), if missing < 4 { 0 } else { 40 });
        assert_eq!(base.yaw.units(), if missing < 5 { 0 } else { 98 });
        assert_eq!(base.roll, Angle::ZERO);
        if missing != 0 {
            assert_eq!(
                fixture
                    .world
                    .player(&fixture.objects, fixture.owner)
                    .unwrap()
                    .pose
                    .unwrap()
                    .yaw_trim,
                48
            );
        }
    }
}

#[test]
fn continuous_double_tap_pose_publishes_post_decay_roll_without_aliasing_protection() {
    for shoulder in [Button::LeftShoulder, Button::RightShoulder] {
        let mut fixture = Fixture::new();
        let bit = shoulder as u16;
        let mut expected_bank = 0i8;
        let mut previous_impulse = 0i8;
        for visit in 0..40 {
            let (held, pressed) = if visit == 0 || visit == 2 {
                (bit, bit)
            } else {
                (0, 0)
            };
            fixture.world.processed_player_input = Some(InputState {
                held: Buttons::from_bits(held),
                pressed: Buttons::from_bits(pressed),
            });
            player_roll::prepare_shoulders(&fixture.objects, &mut fixture.world, fixture.owner)
                .unwrap();
            player_roll::advance(&fixture.objects, &mut fixture.world, fixture.owner).unwrap();
            let impulse = fixture
                .world
                .player(&fixture.objects, fixture.owner)
                .unwrap()
                .roll
                .unwrap()
                .impulse;
            if impulse == 0 {
                let half = expected_bank / 2;
                expected_bank = half + half / 2;
            }
            expected_bank = expected_bank.wrapping_add(impulse);
            fixture.run().unwrap();
            assert_eq!(fixture.storage().bank.units(), expected_bank as u8);
            assert_eq!(
                fixture
                    .objects
                    .get(fixture.owner)
                    .unwrap()
                    .base
                    .roll
                    .units(),
                expected_bank as u8
            );
            let records = fixture
                .world
                .player(&fixture.objects, fixture.owner)
                .unwrap();
            assert_eq!(
                records.protection.unwrap().control() & 0x40 != 0,
                previous_impulse != 0
            );
            previous_impulse = impulse;
        }
    }
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("pose does not dispatch modes")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("pose does not dispatch death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("pose does not resume the map")
    }
}

#[test]
fn scene_failure_latches_before_retry_and_released_storage_cannot_be_reused() {
    let mut fixture = Fixture::new();
    fixture.world.player_yaw_increment = None;
    let mut execution = SceneExecution::default();
    execution.paths.runtime = fixture.runtime;
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut fixture.objects,
        world: &mut fixture.world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 1,
    };
    assert_eq!(
        host.compose_player_pose(fixture.owner),
        Err(SceneError::PlayerPose(PoseError::MissingYawIncrement))
    );
    host.world.player_yaw_increment = Some(2);
    assert_eq!(
        host.compose_player_pose(fixture.owner),
        Err(SceneError::Faulted)
    );
    host.execution
        .paths
        .runtime
        .release_actor_programs(host.objects, fixture.owner)
        .unwrap();
    assert!(matches!(
        player_storage::get_mut(
            host.objects,
            &mut host.execution.paths.runtime.resources,
            fixture.owner
        ),
        Err(PlayerStorageError::MissingStorage(_))
    ));
}
