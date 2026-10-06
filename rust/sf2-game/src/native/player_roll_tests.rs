use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::path_protection::DeflectionProtection;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Buttons, Object, ObjectKind, RandomState, ShapeId};

fn input(held: u16, pressed: u16) -> InputState {
    InputState {
        held: Buttons::from_bits(held),
        pressed: Buttons::from_bits(pressed),
    }
}

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    let mut world = ScenePathWorld::new(RandomState::default());
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                roll: Some(PlayerRoll::default()),
                protection: Some(DeflectionProtection::from_control(0xA5)),
                auxiliary: Some(SelectedAuxiliaryState {
                    action_flags: 0xEF,
                    mode: 0x11,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner)
}

#[test]
fn shoulder_arbitration_exhausts_retained_flags_and_all_edge_and_hold_combinations() {
    for old in 0..=u8::MAX {
        for pressed in 0..4u8 {
            for held in 0..4u8 {
                let mut control = ShoulderControl::from_bits(old);
                control.update(input(
                    u16::from(held) * 16 | 0xFFCF,
                    u16::from(pressed) * 16 | 0xFFCF,
                ));
                let right_preferred = match pressed {
                    0 => old & 0x80 != 0,
                    2 => false,
                    _ => true,
                };
                let selection = match held {
                    0 => 0,
                    1 => 0x20,
                    2 => 0x40,
                    _ if right_preferred => 0x20,
                    _ => 0x40,
                };
                assert_eq!(
                    control.bits(),
                    old & 0x1F | if right_preferred { 0x80 } else { 0 } | selection
                );
            }
        }
    }
}

#[test]
fn double_tap_protection_is_delayed_on_entry_and_retained_on_last_decay_visit() {
    for shoulder in [Button::LeftShoulder, Button::RightShoulder] {
        let (objects, mut world, owner) = fixture();
        let bit = shoulder as u16;
        for (held, pressed) in [(bit, bit), (0, 0), (bit, bit)] {
            world.processed_player_input = Some(input(held, pressed));
            prepare_shoulders(&objects, &mut world, owner).unwrap();
            advance(&objects, &mut world, owner).unwrap();
        }
        let sign = if shoulder == Button::LeftShoulder {
            1
        } else {
            -1
        };
        let records = *world.player(&objects, owner).unwrap();
        assert_eq!(records.roll.unwrap().impulse, sign * 32);
        assert_eq!(records.roll.unwrap().tap_window, bit as u8 | 1);
        assert_eq!(records.auxiliary.unwrap().action_flags, 0xFF);
        assert_eq!(records.protection.unwrap().control(), 0xA5);
        // No new input or auxiliary record is needed during active decay.
        world.processed_player_input = None;
        world.player_mut(&objects, owner).unwrap().auxiliary = None;
        for remaining in (0..=30).rev().step_by(2) {
            advance(&objects, &mut world, owner).unwrap();
            let records = world.player(&objects, owner).unwrap();
            assert_eq!(records.roll.unwrap().impulse, sign * remaining);
            assert_eq!(records.roll.unwrap().tap_window, bit as u8 | 15);
            assert_eq!(records.protection.unwrap().control(), 0xE5);
        }
        world.player_mut(&objects, owner).unwrap().auxiliary = records.auxiliary;
        world.processed_player_input = Some(input(0, 0));
        advance(&objects, &mut world, owner).unwrap();
        let records = world.player(&objects, owner).unwrap();
        assert_eq!(records.roll.unwrap().impulse, 0);
        assert_eq!(records.protection.unwrap().control(), 0xA5);
        assert_eq!(records.auxiliary.unwrap().action_flags, 0xEF);
    }
}

#[test]
fn active_roll_preserves_unrelated_owners_and_wraps_every_signed_impulse() {
    let (objects, mut world, owner) = fixture();
    let before = *world.player(&objects, owner).unwrap();
    for raw in 1..=u8::MAX {
        for protection in 0..=u8::MAX {
            let roll = PlayerRoll {
                shoulders: ShoulderControl::from_bits(raw),
                tap_window: protection,
                impulse: raw as i8,
            };
            let records = world.player_mut(&objects, owner).unwrap();
            *records = before;
            records.roll = Some(roll);
            records.protection = Some(DeflectionProtection::from_control(protection));
            advance(&objects, &mut world, owner).unwrap();
            let mut expected = before;
            expected.roll = Some(PlayerRoll {
                impulse: (i16::from(raw as i8) + if raw >= 128 { 2 } else { -2 }) as i8,
                tap_window: protection | 15,
                ..roll
            });
            expected.protection = Some(DeflectionProtection::from_control(protection | 0x40));
            assert_eq!(*world.player(&objects, owner).unwrap(), expected);
        }
    }
}

#[test]
fn new_roll_requires_matching_tap_younger_than_seven_and_rejects_both_held() {
    let (objects, mut world, owner) = fixture();
    for window in 0..=u8::MAX {
        for pressed in [0, 0x10, 0x20, 0x30] {
            for held in [0, 0x10, 0x20, 0x30] {
                let records = world.player_mut(&objects, owner).unwrap();
                records.roll = Some(PlayerRoll {
                    shoulders: ShoulderControl::from_bits(0x40),
                    tap_window: window,
                    impulse: 0,
                });
                records.auxiliary.as_mut().unwrap().action_flags = 0xFF;
                records.protection = Some(DeflectionProtection::from_control(window));
                world.processed_player_input = Some(input(held, pressed));
                advance(&objects, &mut world, owner).unwrap();
                let expected_start = pressed != 0
                    && held != 0x30
                    && window & 0x30 == pressed as u8
                    && window & 15 < 7;
                let records = world.player(&objects, owner).unwrap();
                let after = records.roll.unwrap();
                assert_eq!(after.impulse, if expected_start { 32 } else { 0 });
                assert_eq!(records.protection.unwrap().control(), window & !0x40);
                assert_eq!(
                    records.auxiliary.unwrap().action_flags,
                    if expected_start { 0xFF } else { 0xEF }
                );
                assert_eq!(after.tap_window & 0xC0, window & 0xC0);
                if pressed == 0 || held == 0x30 {
                    assert_eq!(
                        after.tap_window,
                        (window & 0xF0) | ((window & 15) + 1).min(15)
                    );
                } else if expected_start {
                    assert_eq!(after.tap_window, (window & 0xF0) | 1);
                } else {
                    assert_eq!(after.tap_window, window & 0xC0 | pressed as u8);
                }
            }
        }
    }
}

#[test]
fn missing_input_keeps_protection_and_action_prefix_without_mutating_roll() {
    let (objects, mut world, owner) = fixture();
    world.player_mut(&objects, owner).unwrap().protection =
        Some(DeflectionProtection::from_control(0xFF));
    world
        .player_mut(&objects, owner)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .action_flags = 0xFF;
    let old_roll = world.player(&objects, owner).unwrap().roll;
    assert_eq!(
        advance(&objects, &mut world, owner),
        Err(RollError::MissingProcessedInput)
    );
    let records = world.player(&objects, owner).unwrap();
    assert_eq!(records.roll, old_roll);
    assert_eq!(records.protection.unwrap().control(), 0xBF);
    assert_eq!(records.auxiliary.unwrap().action_flags, 0xEF);
    assert_eq!(
        prepare_shoulders(&objects, &mut world, owner),
        Err(RollError::MissingProcessedInput)
    );
    assert_eq!(world.player(&objects, owner).unwrap().roll, old_roll);
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("roll services cannot invoke unfinished modes")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("roll services cannot invoke death dispatch")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("roll services cannot invoke map continuation")
    }
}

#[test]
fn scene_latches_partial_failure_and_deleted_player_bindings_cannot_be_reused() {
    let (mut objects, mut world, owner) = fixture();
    let mut execution = SceneExecution::default();
    let catalog = PathCatalog::new(Vec::new()).unwrap();
    let mut callbacks = Callbacks;
    {
        let mut host = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 1,
        };
        assert_eq!(
            host.advance_player_roll(owner),
            Err(SceneError::PlayerRoll(RollError::MissingProcessedInput))
        );
        host.world.processed_player_input = Some(input(0, 0));
        assert_eq!(host.advance_player_roll(owner), Err(SceneError::Faulted));
        assert_eq!(
            host.prepare_player_shoulders(owner),
            Err(SceneError::Faulted)
        );
    }
    objects.remove(owner);
    assert!(matches!(
        advance(&objects, &mut world, owner),
        Err(RollError::World(_))
    ));
    let reused = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    assert_eq!(owner, reused);
    assert!(matches!(
        prepare_shoulders(&objects, &mut world, owner),
        Err(RollError::World(WorldInputError::StalePlayerRecord(_)))
    ));
}
