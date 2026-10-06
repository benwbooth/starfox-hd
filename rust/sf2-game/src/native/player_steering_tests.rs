use super::*;
use crate::path_program::PathCatalog;
use crate::path_runtime::PathRuntime;
use crate::player_roll::{self, ShoulderControl};
use crate::player_storage::PlayerStorageInputs;
use crate::scene_path_world::PlayerPathRecords;
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
        world
            .player_mut(&objects, owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = 0x11;
        world.processed_player_input = Some(Default::default());
        Self {
            objects,
            world,
            runtime,
            owner,
        }
    }

    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }

    fn run(&mut self, response: Option<u16>) -> Result<(), SteeringError> {
        advance(
            &self.objects,
            &mut self.world,
            &mut self.runtime.resources,
            self.owner,
            SteeringContext {
                inherited_response_target: response,
            },
        )
    }

    fn input(&mut self, held: u16, pressed: u16) {
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(held),
            pressed: Buttons::from_bits(pressed),
        });
    }
}

#[test]
fn half_chase_wraps_at_word_width_and_makes_progress_toward_half_turn_ties() {
    for current in [0, 1, 32767, 32768, 65535u16] {
        for target in 0..=u16::MAX {
            let delta = target.wrapping_sub(current) as i16;
            let step = if delta.abs_diff(0) < 2 {
                delta
            } else {
                delta / 2
            };
            assert_eq!(
                half_word(current, target),
                current.wrapping_add(step as u16)
            );
        }
    }
}

#[test]
fn neutral_clears_response_without_clearing_lean_or_shared_yaw() {
    let mut fixture = Fixture::new();
    fixture.world.player_yaw_increment = Some(9876);
    fixture.records().pose.as_mut().unwrap().turning_lean = 1234;
    fixture.records().steering.as_mut().unwrap().turn_response = 555;
    fixture.records().steering.as_mut().unwrap().lateral_offset = -3;
    fixture.records().steering.as_mut().unwrap().direction_age = 255;
    fixture.run(None).unwrap();
    assert_eq!(fixture.world.player_yaw_increment, Some(9876));
    assert_eq!(fixture.records().pose.unwrap().turning_lean, 1234);
    let steering = fixture.records().steering.unwrap();
    assert_eq!(steering.turn_response, 0);
    assert_eq!(steering.lateral_offset, -2);
    assert_eq!(steering.direction_age, 255);
}

#[test]
fn all_pilot_codes_direction_precedence_shoulders_and_braking_use_source_tables() {
    for pilot in 0..=u8::MAX {
        for direction in [Button::Left as u16, Button::Right as u16, DIRECTION_MASK] {
            for held_shoulders in [
                0,
                Button::LeftShoulder as u16,
                Button::RightShoulder as u16,
                SHOULDER_MASK,
            ] {
                for braking in [false, true] {
                    let mut fixture = Fixture::new();
                    fixture.records().visit.as_mut().unwrap().pilot_code = pilot;
                    fixture.records().auxiliary.as_mut().unwrap().action_flags =
                        if braking { BRAKING } else { 0 };
                    fixture.input(direction | held_shoulders, direction);
                    fixture.run(None).unwrap();
                    let index = if pilot < 6 { usize::from(pilot) } else { 0 };
                    let left = direction & Button::Left as u16 != 0;
                    let step = if held_shoulders != 0 {
                        if braking {
                            960u16
                        } else {
                            SHOULDER_YAW_STEP[index]
                        }
                    } else {
                        NORMAL_YAW_STEP[index]
                    };
                    assert_eq!(
                        fixture.world.player_yaw_increment,
                        Some(if left { step } else { step.wrapping_neg() })
                    );
                    let steering = fixture.records().steering.unwrap();
                    assert_eq!(
                        steering.direction_age,
                        ((direction >> 2) as u8) + u8::from(left)
                    );
                    assert_eq!(steering.camera_bank_target, if left { -6 } else { 6 });
                    assert_eq!(
                        fixture.records().auxiliary.unwrap().action_flags & SHARP_TURN != 0,
                        braking && held_shoulders != 0
                    );
                    assert_eq!(steering.turn_response, 160);
                }
            }
        }
    }
}

#[test]
fn shoulder_selection_and_contact_gate_do_not_fabricate_or_duplicate_roll_state() {
    for ignored in [false, true] {
        for selected in [0, 0x20, 0x40, 0x60] {
            let mut fixture = Fixture::new();
            fixture.records().contact.as_mut().unwrap().ignores_contacts = ignored;
            fixture.records().roll.as_mut().unwrap().shoulders =
                ShoulderControl::from_bits(selected);
            for initial in i16::MIN..=i16::MAX {
                let mut bank = initial;
                let expected = if ignored {
                    bank = half_word(bank as u16, 0) as i16;
                    chase_word(bank as u16, 0) as i16
                } else if selected & 0x40 != 0 {
                    bank.wrapping_add(10)
                } else if selected & 0x20 != 0 {
                    bank.wrapping_sub(10)
                } else {
                    chase_word(bank as u16, 0) as i16
                };
                fixture.records().pose.as_mut().unwrap().shoulder_bank = initial;
                fixture.run(None).unwrap();
                assert_eq!(
                    fixture.records().pose.unwrap().shoulder_bank,
                    expected.clamp(-64, 64)
                );
            }
            fixture.input(Button::Left as u16, 0);
            fixture.records().contact.as_mut().unwrap().ignores_contacts = ignored;
            fixture.records().roll.as_mut().unwrap().shoulders =
                ShoulderControl::from_bits(selected);
            fixture.records().pose.as_mut().unwrap().shoulder_bank = -63;
            fixture.run(None).unwrap();
            assert_eq!(
                fixture.records().pose.unwrap().shoulder_bank,
                if ignored {
                    -28
                } else if selected & 0x40 != 0 {
                    -53
                } else if selected & 0x20 != 0 {
                    -64
                } else {
                    -56
                }
            );
            assert_eq!(
                fixture.records().steering.unwrap().camera_bank_target,
                if ignored { 0 } else { -6 }
            );
        }
    }
}

#[test]
fn special_modes_require_an_explicit_response_only_for_directional_input() {
    for (mode, flags) in [(0x30, 0), (0x11, HEADING_LOCKED), (0x30, HEADING_LOCKED)] {
        let mut fixture = Fixture::new();
        fixture.records().auxiliary.as_mut().unwrap().mode = mode;
        fixture.records().auxiliary.as_mut().unwrap().action_flags = flags;
        fixture.run(None).unwrap();
        fixture.input(Button::Right as u16, 0);
        let before = *fixture.records();
        assert_eq!(
            fixture.run(None),
            Err(SteeringError::MissingInheritedResponseTarget)
        );
        assert_eq!(*fixture.records(), before);
        fixture.run(Some(1024)).unwrap();
        assert_eq!(fixture.records().steering.unwrap().turn_response, 256);
    }
}

#[test]
fn missing_later_owners_preserve_the_exact_completed_prefix() {
    let mut fixture = Fixture::new();
    fixture.input(Button::Left as u16, 0);
    fixture.records().contact = None;
    assert_eq!(
        fixture.run(None),
        Err(SteeringError::World(WorldInputError::MissingPlayerContact(
            fixture.owner
        )))
    );
    assert_eq!(fixture.records().pose.unwrap().turning_lean, 160);
    assert_eq!(fixture.world.player_yaw_increment, Some(512));
    assert_eq!(fixture.records().pose.unwrap().steering_bank, 0);
    fixture.records().contact = Some(Default::default());
    fixture.records().roll = None;
    assert_eq!(
        fixture.run(None),
        Err(SteeringError::MissingRoll(fixture.owner))
    );
    assert_eq!(fixture.records().pose.unwrap().steering_bank, 3);
    fixture.records().contact.as_mut().unwrap().ignores_contacts = true;
    fixture.run(None).unwrap();
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("not mode dispatch")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("not death dispatch")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("not map dispatch")
    }
}

#[test]
fn scene_wrapper_latches_prefix_failure_and_uses_owned_player_storage() {
    let mut fixture = Fixture::new();
    fixture.input(Button::Left as u16, 0);
    player_roll::prepare_shoulders(&fixture.objects, &mut fixture.world, fixture.owner).unwrap();
    fixture.records().roll = None;
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
        host.advance_player_steering(fixture.owner, Default::default()),
        Err(SceneError::PlayerSteering(SteeringError::MissingRoll(
            fixture.owner
        )))
    );
    host.world
        .player_mut(host.objects, fixture.owner)
        .unwrap()
        .roll = Some(Default::default());
    assert_eq!(
        host.advance_player_steering(fixture.owner, Default::default()),
        Err(SceneError::Faulted)
    );
}
