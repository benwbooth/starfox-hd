use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
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
        objects.get_mut(owner).unwrap().base.behavior_phase = 197;
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    mode_selection: Some(PlayerModeSelection::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x11,
                        action_flags: 0,
                        stored_world_position: Default::default(),
                        stored_rotation: Default::default(),
                    }),
                    boundary: Some(crate::player_boundary::PlayerBoundary {
                        return_position: crate::Vector3 {
                            x: -312,
                            y: 17,
                            z: 321,
                        },
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        Self {
            objects,
            world,
            execution: Default::default(),
            owner,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn state(&mut self) -> &mut PlayerModeSelection {
        self.records().mode_selection.as_mut().unwrap()
    }
    fn phase(&self) -> u8 {
        self.objects.get(self.owner).unwrap().base.behavior_phase
    }
    fn advance(&mut self, request: ModeRequest) -> Result<bool, SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .select_player_mode(self.owner, request)
    }
}

#[test]
fn request_inhibition_does_not_block_pending_transform_or_require_new_input() {
    let mut f = Fixture::new();
    f.state().set_request_inhibited(true);
    f.state().requested = 0xA4;
    assert_eq!(f.advance(ModeRequest::RetainedPitch), Ok(true));
    assert_eq!(f.phase(), ENTER_WALKER);
    assert_eq!(f.state().requested, 0xA4);
    assert_eq!(f.records().auxiliary.unwrap().mode, 0x11);
}

#[test]
fn absent_pending_request_does_not_require_current_mode_when_gate_is_closed() {
    let mut f = Fixture::new();
    f.state().set_request_inhibited(true);
    f.state().requested = 0xD0;
    f.records().auxiliary = None;
    assert_eq!(f.advance(ModeRequest::Walker), Ok(false));
    assert_eq!(f.phase(), 197);
    f.state().set_request_inhibited(false);
    f.world.player_carry_mode = Some(1);
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .carry_selected_player = true;
    // Carry mismatch skips the input dependency but not existing requests.
    assert_eq!(f.advance(ModeRequest::Walker), Ok(false));
}

#[test]
fn same_family_selects_initializer_without_transform_cue_or_pending_flags_loss() {
    let mut f = Fixture::new();
    f.state().set_request_inhibited(true);
    f.state().requested = 0xB2;
    f.state().transition_control = 0xFF;
    let return_position = f.records().boundary.unwrap().return_position;
    assert_eq!(f.advance(ModeRequest::Walker), Ok(false));
    assert_eq!(f.phase(), 7);
    assert_eq!(f.state().requested, 0xB0);
    assert_eq!(f.state().transition_control, 0xFF);
    assert_eq!(
        f.records().boundary.unwrap().return_position,
        return_position
    );
    assert_eq!(f.records().auxiliary.unwrap().mode, 0x11);
}

#[test]
fn queued_transform_converts_only_control_bits_and_preserves_adjacent_return_position() {
    let mut f = Fixture::new();
    f.state().set_request_inhibited(true);
    f.state().requested = 0xC4;
    f.state().transition_control = 0xF7;
    let return_position = f.records().boundary.unwrap().return_position;
    assert_eq!(f.advance(ModeRequest::RetainedPitch), Ok(true));
    assert_eq!(f.phase(), ENTER_WALKER);
    assert_eq!(f.state().transition_control, 0xEF);
    assert_eq!(
        f.records().boundary.unwrap().return_position,
        return_position
    );
    assert_eq!(f.advance(ModeRequest::RetainedPitch), Ok(false));
    assert_eq!(f.phase(), ENTER_WALKER);
}

#[test]
fn admitted_select_replaces_request_before_missing_mode_fault_and_never_replays() {
    let mut f = Fixture::new();
    f.state().requested = 0xA1;
    f.records().auxiliary = None;
    f.world.player_carry_mode = Some(0);
    f.world.processed_player_input = Some(InputState {
        held: Default::default(),
        pressed: Buttons::from_bits(Button::Select as u16),
    });
    assert_eq!(
        f.advance(ModeRequest::Walker),
        Err(SceneError::PlayerModeSelection(ModeSelectionError::World(
            WorldInputError::MissingAuxiliary(f.owner)
        )))
    );
    assert_eq!(f.state().requested, 0xA4);
    assert_eq!(f.phase(), 197);
    assert_eq!(f.advance(ModeRequest::FreeFlight), Err(SceneError::Faulted));
    assert_eq!(f.state().requested, 0xA4);
}

#[test]
fn invalid_pending_selector_is_diagnosed_only_when_original_would_index_table() {
    let mut f = Fixture::new();
    f.state().set_request_inhibited(true);
    f.state().requested = 0xF7;
    f.records().auxiliary.as_mut().unwrap().mode = 0x17;
    assert_eq!(f.advance(ModeRequest::Walker), Ok(false));
    assert_eq!(f.phase(), 197);
    f.records().auxiliary.as_mut().unwrap().mode = 0x11;
    assert_eq!(
        f.advance(ModeRequest::Walker),
        Err(SceneError::PlayerModeSelection(
            ModeSelectionError::UnsupportedPendingMode(7)
        ))
    );
    assert_eq!(f.phase(), 197);
}
