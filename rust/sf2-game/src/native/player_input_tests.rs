use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_transition::ViewTransitionMode;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

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
    let mut world = ScenePathWorld::new(RandomState::new([3, 7, 13, 31]));
    world.view_transition_mode = Some(ViewTransitionMode::default());
    world.controller_inputs = [Some(input(0xABCD, 0x5A96)), Some(input(0x7531, 0xB694))];
    world.player_input_settings = Some(PlayerInputSettings::default());
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                contact: Some(Default::default()),
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 16,
                    action_flags: 1,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                injected_input: Some(InputState::default()),
                ..Default::default()
            },
        )
        .unwrap();
    (objects, world, owner)
}

#[test]
fn every_input_word_uses_exact_per_button_remapping_and_keeps_unmapped_bits() {
    let inversion = [(Button::Up, Button::Down), (Button::Down, Button::Up)];
    let alternate = [
        (Button::A, Button::X),
        (Button::B, Button::A),
        (Button::X, Button::Y),
        (Button::Y, Button::B),
    ];
    let reference = |word, mapping: &[(Button, Button)]| {
        let mask = mapping
            .iter()
            .fold(0, |mask, (from, _)| mask | *from as u16);
        mapping.iter().fold(word & !mask, |mapped, (from, to)| {
            if word & *from as u16 != 0 {
                mapped | *to as u16
            } else {
                mapped
            }
        })
    };
    for word in 0..=u16::MAX {
        assert_eq!(invert_vertical(word), reference(word, &inversion));
        assert_eq!(alternate_buttons(word), reference(word, &alternate));
    }
}

#[test]
fn every_mode_and_option_byte_use_actor_side_and_distinct_publications() {
    let (mut objects, mut world, owner) = fixture();
    // Primary selection intentionally contradicts the actual side in half
    // the cases; it must not choose the controller.
    world.primary_player = Some(owner);
    for mode in 0..=u8::MAX {
        for option in 0..=u8::MAX {
            let secondary = option & 1 != 0;
            objects.get_mut(owner).unwrap().base.contacts.hit_side = if secondary {
                HitSide::Secondary
            } else {
                HitSide::Primary
            };
            let active = option & 2 != 0;
            let records = world.player_mut(&objects, owner).unwrap();
            records.auxiliary.as_mut().unwrap().mode = mode;
            records.auxiliary.as_mut().unwrap().action_flags = if active { 0xF1 } else { 0xFE };
            records.injected_input = Some(input(0x0840, 0x0440));
            world.player_input_settings = Some(PlayerInputSettings {
                flight_style: option,
                button_layout: option,
            });
            let mut expected = world.controller_inputs[usize::from(secondary)].unwrap();
            if (16..32).contains(&mode) && option >= 128 {
                expected = map_words(expected, invert_vertical);
            }
            if option != 0 {
                expected = map_words(expected, alternate_buttons);
            }
            let unmasked = expected;
            if !active {
                expected = map_words(expected, |word| word & 0x1000);
            }
            expected.held = Buttons::from_bits(expected.held.bits() | 0x0840);
            expected.pressed = Buttons::from_bits(expected.pressed.bits() | 0x0440);
            assert_eq!(prepare(&objects, &mut world, owner), Ok(expected));
            assert_eq!(world.processed_player_input, Some(expected));
            assert_eq!(world.unmasked_player_input, Some(unmasked));
            assert_eq!(
                world.player(&objects, owner).unwrap().injected_input,
                Some(InputState::default())
            );
            assert_eq!(world.random.bytes(), [3, 7, 13, 31]);
        }
    }
}

#[test]
fn gating_retains_injection_and_pre_mask_publication_without_reading_later_inputs() {
    for scripted in [true, false] {
        let (objects, mut world, owner) = fixture();
        world
            .view_transition_mode
            .as_mut()
            .unwrap()
            .set_active(scripted);
        world
            .player_mut(&objects, owner)
            .unwrap()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = true;
        world.player_mut(&objects, owner).unwrap().injected_input = Some(input(0x0909, 0xA0A0));
        if scripted {
            world.player_mut(&objects, owner).unwrap().contact = None;
        }
        world.player_mut(&objects, owner).unwrap().auxiliary = None;
        world.controller_inputs = [None; 2];
        world.player_input_settings = None;
        world.processed_player_input = Some(input(0xAAAA, 0x5555));
        world.unmasked_player_input = Some(input(0x1357, 0x2468));
        assert_eq!(
            prepare(&objects, &mut world, owner),
            Ok(InputState::default())
        );
        assert_eq!(world.processed_player_input, Some(InputState::default()));
        assert_eq!(world.unmasked_player_input, Some(input(0x1357, 0x2468)));
        assert_eq!(
            world.player(&objects, owner).unwrap().injected_input,
            Some(input(0x0909, 0xA0A0))
        );
    }
}

#[test]
fn controller_edges_are_not_resampled_and_script_input_is_consumed_once() {
    let (objects, mut world, owner) = fixture();
    world.controller_inputs[0] = Some(input(0x2000, 0x1000));
    world.player_mut(&objects, owner).unwrap().injected_input = Some(input(0x4000, 0x8000));
    assert_eq!(
        prepare(&objects, &mut world, owner).unwrap(),
        input(0x6000, 0x9000)
    );
    assert_eq!(
        prepare(&objects, &mut world, owner).unwrap(),
        input(0x2000, 0x1000)
    );
    assert_eq!(
        prepare(&objects, &mut world, owner).unwrap(),
        input(0x2000, 0x1000)
    );
    assert_eq!(world.controller_inputs[0], Some(input(0x2000, 0x1000)));
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("input preparation never executes a player mode")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("unexpected death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("unexpected map continuation")
    }
}

#[test]
fn missing_inputs_preserve_ordered_prefixes_and_scene_fault_prevents_replay() {
    for missing in 0..6 {
        let (mut objects, mut world, owner) = fixture();
        let raw = world.controller_inputs[0].unwrap();
        world.unmasked_player_input = Some(input(0x1357, 0x2468));
        world
            .player_mut(&objects, owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .action_flags = 0;
        let expected = match missing {
            0 => {
                world.view_transition_mode = None;
                PlayerInputError::MissingScriptedViewMode
            }
            1 => {
                world.player_mut(&objects, owner).unwrap().contact = None;
                PlayerInputError::World(WorldInputError::MissingPlayerContact(owner))
            }
            2 => {
                world.controller_inputs[0] = None;
                PlayerInputError::MissingController(HitSide::Primary)
            }
            3 => {
                world.player_mut(&objects, owner).unwrap().auxiliary = None;
                PlayerInputError::World(WorldInputError::MissingAuxiliary(owner))
            }
            4 => {
                world.player_input_settings = None;
                PlayerInputError::MissingSettings
            }
            _ => {
                world.player_mut(&objects, owner).unwrap().injected_input = None;
                PlayerInputError::MissingInjectedInput(owner)
            }
        };
        let mut execution = SceneExecution::default();
        let catalog = PathCatalog::new(vec![]).unwrap();
        let mut callbacks = Callbacks;
        let mut host = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            catalog: &catalog,
            callbacks: &mut callbacks,
            statement_budget: 1,
        };
        assert_eq!(
            host.prepare_player_input(owner),
            Err(SceneError::PlayerInput(expected))
        );
        let processed = if missing < 3 {
            InputState::default()
        } else if missing < 5 {
            raw
        } else {
            map_words(raw, |word| word & 0x1000)
        };
        assert_eq!(host.world.processed_player_input, Some(processed));
        assert_eq!(
            host.world.unmasked_player_input,
            Some(if missing < 5 {
                input(0x1357, 0x2468)
            } else {
                raw
            })
        );
        host.world.processed_player_input = Some(input(0x5A5A, 0xA5A5));
        assert_eq!(host.prepare_player_input(owner), Err(SceneError::Faulted));
        assert_eq!(
            host.world.processed_player_input,
            Some(input(0x5A5A, 0xA5A5))
        );
    }
}
