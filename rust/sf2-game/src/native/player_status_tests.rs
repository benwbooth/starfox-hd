use super::*;
use crate::path_program::PathCatalog;
use crate::player_mode_selection::{self, ModeRequest, ModeSelectionError};
use crate::player_storage::PlayerStorageInputs;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

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
        let mut world = ScenePathWorld::new(RandomState::default());
        let mut execution = SceneExecution::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut execution.paths.runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 19,
                score: Default::default(),
            },
        )
        .unwrap();
        world.active_shield_capacity = Some(32);
        world.view_transition_mode = Some(Default::default());
        world.player_carry_mode = Some(1);
        Self {
            objects,
            world,
            execution,
            owner,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn storage(&mut self) -> &mut player_storage::PlayerStorage {
        player_storage::get_mut(
            &self.objects,
            &mut self.execution.paths.runtime.resources,
            self.owner,
        )
        .unwrap()
    }
    fn cues(&mut self) -> Vec<u16> {
        self.world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .map(|event| {
                let SoundEvent::Authored(cue) = event else {
                    panic!("unexpected cue")
                };
                u16::from(cue.id)
                    | u16::from(cue.parameter()) << 8
                    | if cue.target == PlayerTarget::Secondary {
                        0x8000
                    } else {
                        0
                    }
            })
            .collect()
    }
    fn shield(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .advance_player_shield_status(self.owner)
    }
    fn filters(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 64,
        }
        .advance_player_status_filters(self.owner)
    }
}

#[test]
fn shield_display_all_bytes_wait_for_real_acknowledgement_and_preserve_capacity_boundaries() {
    let mut f = Fixture::new();
    for displayed in 0..=u8::MAX {
        for shield in 0..=u8::MAX {
            f.storage().retained_shield = displayed;
            f.records().contact.as_mut().unwrap().hit.reserve_shield = shield;
            f.records().status.as_mut().unwrap().shield_warning_history = 0;
            let capacity = shield.rotate_left(3);
            f.world.active_shield_capacity = Some(capacity);
            f.shield().unwrap();
            let expected = if displayed >= 128 || displayed == shield {
                displayed
            } else if shield < displayed {
                (displayed - 1) | 128
            } else {
                displayed.wrapping_add(1).min(capacity) | 128
            };
            assert_eq!(f.storage().retained_shield, expected);
            assert_eq!(f.records().status.unwrap().shield_warning_history, shield);
            assert!(f.cues().is_empty());
        }
    }
}

#[test]
fn shield_warning_all_previous_and_live_bytes_use_unsigned_loss_and_actual_caller_side() {
    let mut f = Fixture::new();
    for alive in [false, true] {
        for primary in [false, true] {
            f.world.primary_player = primary.then_some(f.owner);
            f.objects.get_mut(f.owner).unwrap().base.hit_points = u8::from(alive);
            for previous in 0..=u8::MAX {
                for shield in 0..=u8::MAX {
                    f.records().contact.as_mut().unwrap().hit.reserve_shield = shield;
                    f.records().status.as_mut().unwrap().shield_warning_history = previous;
                    warn_shield_loss(&f.objects, &mut f.world, f.owner).unwrap();
                    let expected = if alive && shield < previous && shield < 13 {
                        vec![(if shield < 5 { 23 } else { 22 }) | if primary { 0 } else { 0x8000 }]
                    } else {
                        vec![]
                    };
                    assert_eq!(f.cues(), expected);
                    assert_eq!(
                        f.records().status.unwrap().shield_warning_history,
                        if alive { shield } else { previous }
                    );
                }
            }
        }
    }
}

#[test]
fn pending_display_does_not_suppress_new_damage_warning_and_never_acknowledges_itself() {
    let mut f = Fixture::new();
    f.storage().retained_shield = 0xA0;
    f.records().contact.as_mut().unwrap().hit.reserve_shield = 7;
    f.records().status.as_mut().unwrap().shield_warning_history = 9;
    f.world.active_shield_capacity = None;
    f.shield().unwrap();
    assert_eq!(f.cues(), vec![22]);
    assert_eq!(f.storage().retained_shield, 0xA0);
    f.shield().unwrap();
    assert!(f.cues().is_empty());
    assert_eq!(f.storage().retained_shield, 0xA0);
    // The display owner's later acknowledgement allows one next step.
    f.storage().retained_shield &= 0x7F;
    f.shield().unwrap();
    assert_eq!(f.storage().retained_shield, 0x9F);
}

#[test]
fn missing_shield_history_retains_completed_display_mutation_and_faults_the_scene() {
    let mut f = Fixture::new();
    f.storage().retained_shield = 19;
    f.records().contact.as_mut().unwrap().hit.reserve_shield = 7;
    f.records().status = None;
    assert_eq!(
        f.shield(),
        Err(SceneError::PlayerStatus(StatusError::MissingState(f.owner)))
    );
    assert_eq!(f.storage().retained_shield, 0x92);
    f.records().status = Some(Default::default());
    assert_eq!(f.shield(), Err(SceneError::Faulted));
    assert_eq!(f.storage().retained_shield, 0x92);
}

#[test]
fn contact_filters_change_the_real_hit_marker_before_lazy_pause_and_pose_dependencies() {
    let mut f = Fixture::new();
    f.records().contact.as_mut().unwrap().hit.recovery = 2;
    f.records()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .secondary_protection = 3;
    f.records().pose.as_mut().unwrap().heading_return_bank = -10;
    f.world.strategy_clock = 1;
    f.world.view_transition_mode = None;
    f.world.spawn_defaults = None;
    assert_eq!(
        f.filters(),
        Err(SceneError::PlayerStatus(StatusError::MissingViewMode))
    );
    assert_eq!(f.records().contact.unwrap().hit.recovery, 1);
    assert_eq!(f.records().contact.unwrap().hit.secondary_protection, 3);
    assert!(f.objects.get(f.owner).unwrap().base.contacts.hit_marked);
    assert!(
        f.objects
            .get(f.owner)
            .unwrap()
            .base
            .flags
            .collision_disabled
    );
    assert_eq!(f.records().pose.unwrap().heading_return_bank, -10);
    assert_eq!(f.filters(), Err(SceneError::Faulted));
    assert_eq!(f.records().contact.unwrap().hit.recovery, 1);
}

#[test]
fn filters_preserve_tagged_zero_and_chase_actual_heading_return_bank() {
    let mut f = Fixture::new();
    f.records().contact.as_mut().unwrap().hit.recovery = 0x81;
    f.records().pose.as_mut().unwrap().heading_return_bank = -10;
    f.world.strategy_clock = 3;
    f.filters().unwrap();
    assert_eq!(f.records().contact.unwrap().hit.recovery, 0x80);
    assert!(
        f.objects
            .get(f.owner)
            .unwrap()
            .base
            .flags
            .collision_disabled
    );
    assert_eq!(f.records().pose.unwrap().heading_return_bank, -9);
    f.filters().unwrap();
    assert_eq!(f.records().contact.unwrap().hit.recovery, 0x80);
    assert!(
        !f.objects
            .get(f.owner)
            .unwrap()
            .base
            .flags
            .collision_disabled
    );
    assert!(f.objects.get(f.owner).unwrap().base.contacts.hit_marked);
    assert_eq!(f.records().pose.unwrap().heading_return_bank, -8);
}

#[test]
fn transform_cues_cover_every_byte_without_touching_unrelated_mode_state() {
    let mut f = Fixture::new();
    for primary in [false, true] {
        for material in [0, 1, 2, 255] {
            for carried in [false, true] {
                f.world.primary_player = primary.then_some(f.owner);
                f.world.player_carry_mode = Some(material);
                f.objects
                    .get_mut(f.owner)
                    .unwrap()
                    .extension
                    .path_state
                    .motion
                    .carry_selected_player = carried;
                for initial in 0..=u8::MAX {
                    f.records().mode_selection.as_mut().unwrap().cue_control = initial;
                    let before = *f.records();
                    advance_transform_cues(&f.objects, &mut f.world, f.owner).unwrap();
                    let mut expected = before;
                    let mut next = initial;
                    let mut cues = vec![];
                    if initial & 63 != 0 {
                        next -= 1;
                        if next & 63 == 61 {
                            cues.push(
                                (if next & 128 == 0 { 30 } else { 31 })
                                    | if primary { 0 } else { 0x8000 },
                            );
                            next &= !64;
                        }
                        if next & 128 != 0 && next & 63 == 55 && material == 1 && carried {
                            cues.push(169 | if primary { 0 } else { 0x8000 });
                        }
                    }
                    expected.mode_selection.as_mut().unwrap().cue_control = next;
                    assert_eq!(*f.records(), expected);
                    assert_eq!(f.cues(), cues);
                }
            }
        }
    }
}

#[test]
fn cue_completion_unlocks_the_same_request_gate_read_by_mode_selection() {
    let mut f = Fixture::new();
    f.records().mode_selection.as_mut().unwrap().cue_control = 0x7E;
    assert_eq!(
        player_mode_selection::advance(&mut f.objects, &mut f.world, f.owner, ModeRequest::Walker),
        Ok(false)
    );
    advance_transform_cues(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(f.records().mode_selection.unwrap().cue_control, 61);
    assert_eq!(f.cues(), vec![30]);
    assert_eq!(
        player_mode_selection::advance(&mut f.objects, &mut f.world, f.owner, ModeRequest::Walker),
        Err(ModeSelectionError::MissingProcessedInput)
    );
}
