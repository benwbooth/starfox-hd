use super::*;
use crate::hit_response::HitSide;
use crate::player_flight_prepare::{self, FlightPreparationError as PrepareError};
use crate::player_input::{PlayerInputError, PlayerInputSettings};
use crate::player_roll::{PlayerRoll, RollError};
use crate::{Button, Buttons, InputState};

fn prepared_fixture(style: Option<TrackingStyle>) -> Fixture {
    let mut f = fixture(style);
    f.records().roll = Some(PlayerRoll::default());
    f.records().injected_input = Some(InputState::default());
    f.world.controller_inputs = [Some(InputState::default()); 2];
    f.world.player_input_settings = Some(PlayerInputSettings::default());
    f.records().contact.as_mut().unwrap().ignores_contacts = true;
    f.records().camera_ground.as_mut().unwrap().animation_pitch = -99;
    f
}

fn prepare(f: &mut Fixture) -> Result<(), PrepareError> {
    player_flight_prepare::prepare(
        &mut f.objects,
        &mut f.world,
        &mut f.execution.paths.runtime,
        f.owner,
    )
}

#[test]
fn flight_prepare_installs_camera_before_controls_and_uses_new_selector_for_pitch_gate() {
    let mut f = prepared_fixture(Some(TrackingStyle::Surface));
    f.objects.get_mut(f.owner).unwrap().base.contacts.hit_side = HitSide::Secondary;
    f.world.controller_inputs[1] = Some(InputState {
        held: Buttons::from_bits(Button::LeftShoulder as u16 | Button::RightShoulder as u16),
        pressed: Buttons::from_bits(Button::RightShoulder as u16),
    });
    f.records().injected_input = Some(InputState {
        held: Buttons::from_bits(Button::A as u16),
        pressed: Buttons::from_bits(Button::B as u16),
    });
    let position = f.objects.get(f.owner).unwrap().base.position;
    prepare(&mut f).unwrap();
    assert_eq!(
        f.records().camera_dispatch.unwrap().style,
        Some(TrackingStyle::Normal)
    );
    assert_eq!(f.records().camera_ground.unwrap().animation_pitch, -99);
    assert!(!f.records().contact.unwrap().ignores_contacts);
    assert!(
        f.objects
            .get(f.owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
    assert!(f.records().roll.unwrap().shoulders.right_selected());
    assert!(f
        .world
        .processed_player_input
        .unwrap()
        .held
        .contains(Button::A));
    assert_eq!(f.records().injected_input, Some(InputState::default()));
    assert_eq!(f.objects.get(f.owner).unwrap().base.position, position);
}

#[test]
fn flight_prepare_protection_defers_camera_and_consumption_but_runs_ground_pitch() {
    let mut f = prepared_fixture(None);
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    let injected = InputState {
        held: Buttons::from_bits(u16::MAX),
        pressed: Buttons::from_bits(u16::MAX),
    };
    f.records().injected_input = Some(injected);
    f.world.controller_inputs = [None; 2];
    f.world.player_input_settings = None;
    prepare(&mut f).unwrap();
    assert_eq!(f.records().camera_dispatch.unwrap().style, None);
    assert_eq!(f.world.processed_player_input, Some(InputState::default()));
    assert_eq!(f.records().injected_input, Some(injected));
    assert_eq!(f.records().camera_ground.unwrap().animation_pitch, 0);
    assert!(!f.records().roll.unwrap().shoulders.left_selected());
    assert!(!f.records().roll.unwrap().shoulders.right_selected());
}

#[test]
fn flight_prepare_later_missing_records_retain_input_and_contact_effects() {
    let mut f = prepared_fixture(Some(TrackingStyle::Normal));
    f.records().roll = None;
    assert_eq!(
        prepare(&mut f),
        Err(PrepareError::Shoulders(RollError::MissingRoll(f.owner)))
    );
    assert!(!f.records().contact.unwrap().ignores_contacts);
    assert!(
        f.objects
            .get(f.owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
    assert_eq!(f.records().injected_input, Some(InputState::default()));

    let mut f = prepared_fixture(Some(TrackingStyle::Normal));
    f.world.controller_inputs[0] = None;
    assert_eq!(
        prepare(&mut f),
        Err(PrepareError::Input(PlayerInputError::MissingController(
            HitSide::Primary
        )))
    );
    assert!(!f.records().contact.unwrap().ignores_contacts);
    assert_eq!(f.records().camera_ground.unwrap().animation_pitch, -99);
    assert_eq!(f.world.processed_player_input, Some(InputState::default()));
}

#[test]
fn flight_prepare_scene_fault_prevents_repeating_partial_installer_and_input() {
    let mut f = prepared_fixture(None);
    f.world.controller_inputs[0] = None;
    let error = SceneError::PlayerFlightPreparation(PrepareError::Input(
        PlayerInputError::MissingController(HitSide::Primary),
    ));
    let catalog = PathCatalog::new(vec![]).unwrap();
    let mut callbacks = Callbacks;
    let mut scene = SceneActors::<Callbacks> {
        objects: &mut f.objects,
        world: &mut f.world,
        execution: &mut f.execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 10,
    };
    assert_eq!(scene.prepare_player_free_flight(f.owner), Err(error));
    scene.world.controller_inputs[0] = Some(InputState::default());
    scene
        .objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .path_state
        .conditions
        .hit_event_pending = false;
    assert_eq!(
        scene.prepare_player_free_flight(f.owner),
        Err(SceneError::Faulted)
    );
    assert!(
        !scene
            .objects
            .get(f.owner)
            .unwrap()
            .extension
            .path_state
            .conditions
            .hit_event_pending
    );
}
