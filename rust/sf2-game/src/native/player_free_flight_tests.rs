use super::*;
use crate::player_free_flight::{FreeFlightContext, FreeFlightError};
use crate::player_mode_selection::ModeSelectionError;
use crate::player_surface_effect::{SurfaceEffectError, SurfaceEffectInputs};

impl Fixture {
    fn free_flight(&mut self, context: FreeFlightContext) -> Result<FlightResult, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 64,
        }
        .advance_player_free_flight(self.owner, context)
    }

    fn protected(&mut self) -> FreeFlightContext {
        self.records()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .hold_secondary_protection = true;
        self.records().auxiliary.as_mut().unwrap().action_flags = 4;
        self.world.processed_player_input = Some(InputState {
            held: Buttons::from_bits(Button::Left as u16),
            pressed: Default::default(),
        });
        let origin = self.world.weapons.unwrap().fallback.unwrap();
        FreeFlightContext {
            mode: FlightModeContext {
                steering: SteeringContext {
                    inherited_response_target: Some(888),
                },
                flight: Default::default(),
            },
            protected_effect: SurfaceEffectInputs {
                origin: Some(origin),
                lateral: Some(0),
                vertical: Some(19),
                forward: Some(0),
            },
        }
    }
}

#[test]
fn free_flight_surface_publication_precedes_missing_protection_and_input_history() {
    let mut f = Fixture::new();
    f.world.environment_plane_height = Some(1234);
    f.records().contact = None;
    f.records().vertical.as_mut().unwrap().previous_held = 0xABCD;
    assert_eq!(
        f.free_flight(Default::default()),
        Err(SceneError::PlayerFreeFlight(
            FreeFlightError::MissingContact(f.owner)
        ))
    );
    assert_eq!(f.records().surface.unwrap().plane_height, 1234);
    assert_eq!(f.world.surface_clipping_plane_height, Some(1234));
    assert_eq!(f.records().vertical.unwrap().previous_held, 0xABCD);
    assert_eq!(f.world.player_pitch_target, Some(0x1234));
    assert_eq!(f.free_flight(Default::default()), Err(SceneError::Faulted));
}

#[test]
fn free_flight_effect_failure_retains_allocation_without_replaying_on_retry() {
    let mut f = Fixture::new();
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .hold_secondary_protection = true;
    f.records().vertical.as_mut().unwrap().previous_held = 0xABCD;
    assert_eq!(
        f.free_flight(Default::default()),
        Err(SceneError::PlayerFreeFlight(
            FreeFlightError::SurfaceEffect(SurfaceEffectError::MissingOrigin)
        ))
    );
    assert_eq!(f.objects.len(), 3);
    assert_eq!(f.records().vertical.unwrap().previous_held, 0xABCD);
    assert_eq!(f.world.player_pitch_target, Some(0x1234));
    assert_eq!(f.free_flight(Default::default()), Err(SceneError::Faulted));
    assert_eq!(f.objects.len(), 3);
}

#[test]
fn free_flight_rejected_plane_preserves_effect_input_and_effect_supplies_steering() {
    let mut f = Fixture::new();
    let context = f.protected();
    f.world.surface_clipping_plane_height = Some(71);
    f.free_flight(context).unwrap();
    assert_eq!(f.world.surface_clipping_plane_height, Some(71));
    assert_eq!(f.records().steering.unwrap().turn_response, 4);
    let child = f.objects.get(f.owner).unwrap().base.attachment_next.unwrap();
    let effect = f.objects.get(child).unwrap();
    assert!((16..=23).contains(&effect.base.position.y));
    assert_eq!(effect.base.target_speed, 10);
    assert_eq!(
        effect.base.behavior,
        Behavior::SurfaceEffect(crate::player_surface_effect::SurfaceEffectPhase::Initialize)
    );
}

#[test]
fn free_flight_zero_height_support_replaces_missing_effect_input_and_caller_response() {
    let mut f = Fixture::new();
    let mut context = f.protected();
    context.mode.steering.inherited_response_target = None;
    context.protected_effect.vertical = None;
    let origin = context.protected_effect.origin.unwrap();
    let contact = &mut f
        .objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .surface_contact;
    contact.supporting_object = Some(origin);
    contact.flags = 1;
    f.free_flight(context).unwrap();
    assert_eq!(f.world.surface_clipping_plane_height, Some(0));
    assert_eq!(f.records().steering.unwrap().turn_response, 0);
    assert!(f.objects.get(f.owner).unwrap().base.attachment_next.is_some());
}

#[test]
fn free_flight_skipped_effect_preserves_steering_input_and_does_not_require_effect_arguments() {
    let mut f = Fixture::new();
    let mut context = f.protected();
    f.records().auxiliary.as_mut().unwrap().mode = 0x30;
    context.protected_effect = Default::default();
    context.mode.flight.surface.alternate_thrust_target = Some(31);
    let random = f.world.random.clone();
    f.free_flight(context).unwrap();
    assert_eq!(f.records().steering.unwrap().turn_response, 222);
    assert_eq!(f.world.random, random);
    assert_eq!(f.objects.len(), 2);
}

#[test]
fn free_flight_pending_request_uses_actual_side_and_never_retained_pitch_select_cue() {
    for primary in [false, true] {
        let mut f = Fixture::new();
        f.world.primary_player = primary.then_some(f.owner);
        f.records().mode_selection.as_mut().unwrap().requested = 4;
        f.free_flight(Default::default()).unwrap();
        assert_eq!(f.objects.get(f.owner).unwrap().base.behavior_phase, 9);
        assert_eq!(f.records().auxiliary.unwrap().mode, 0x11);
        let events: Vec<_> = f.world.audio.take_events().into_iter().flatten().collect();
        assert_eq!(
            events.last(),
            Some(&SoundEvent::Authored(AuthoredCue::new(
                28,
                0,
                if primary {
                    PlayerTarget::Primary
                } else {
                    PlayerTarget::Secondary
                }
            )))
        );
        assert!(!events
            .iter()
            .any(|event| matches!(event, SoundEvent::Authored(cue) if cue.id == 54)));
    }
}

#[test]
fn free_flight_late_selection_failure_retains_complete_movement_prefix_once() {
    let mut f = Fixture::new();
    f.records().mode_selection = None;
    f.records().roll.as_mut().unwrap().impulse = 32;
    assert_eq!(
        f.free_flight(Default::default()),
        Err(SceneError::PlayerFreeFlight(
            FreeFlightError::ModeSelection(ModeSelectionError::MissingSelection(f.owner))
        ))
    );
    let position = f.objects.get(f.owner).unwrap().base.position;
    assert_ne!(position.z, 0);
    assert_eq!(f.records().roll.unwrap().impulse, 30);
    assert_eq!(f.free_flight(Default::default()), Err(SceneError::Faulted));
    assert_eq!(f.records().roll.unwrap().impulse, 30);
    assert_eq!(f.objects.get(f.owner).unwrap().base.position, position);
}
