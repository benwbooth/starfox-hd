use super::*;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::view_blend::{self, ViewBlendControl};
use crate::view_transition::ViewTransitionMode;
use crate::weapon_dispatch::WeaponState;
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
    view: ObjectId,
}
impl Fixture {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut allocate = || {
            objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap()
        };
        let view = allocate();
        let owner = allocate();
        let proxy = allocate();
        let mut world = ScenePathWorld::new(RandomState::default());
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: FLIGHT_FAMILY,
                        action_flags: 0,
                        stored_rotation: Default::default(),
                        stored_world_position: Default::default(),
                    }),
                    charge: Some(Default::default()),
                    contact: Some(Default::default()),
                    view_distance: Some(Default::default()),
                    ..Default::default()
                },
            )
            .unwrap();
        world.fixed_players[0] = Some(view);
        world.player_view_options_enabled = Some(true);
        world.scene.player_view_control = Some(0);
        world.view_transition_mode = Some(ViewTransitionMode::default());
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            ..Default::default()
        });
        initialize(&objects, &mut world, owner).unwrap();
        Self {
            objects,
            world,
            execution: Default::default(),
            owner,
            view,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn advance(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            callbacks: &mut Callbacks,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            statement_budget: 64,
        }
        .advance_player_view_distance(self.owner)
    }
    fn capture(&self) -> ViewBlendControl {
        ViewBlendControl::capture(self.objects.get(self.view).unwrap())
    }
}

#[test]
fn profiles_replace_only_distance_words_and_preserve_live_linked_state_for_every_mode() {
    for mode in 0..=u8::MAX {
        let mut f = Fixture::new();
        f.records().auxiliary.as_mut().unwrap().mode = mode;
        f.records().charge.as_mut().unwrap().linked_mode = true;
        f.records().charge.as_mut().unwrap().linked_muzzle_disabled = true;
        *f.records().view_distance.as_mut().unwrap() = PlayerViewDistance {
            distance: 123,
            boost_response: 123,
            brake_response: 123,
            linked_target: 123,
            external_target: 123,
            reserved_profile: [123; 2],
            pitch_height_offset: 123,
            capture_pending: true,
            switch_requested: true,
        };
        let before = *f.records();
        initialize(&f.objects, &mut f.world, f.owner).unwrap();
        let state = f.records().view_distance.unwrap();
        let expected = match mode & MODE_FAMILY_MASK {
            FLIGHT_FAMILY => (-240, -50, 40, 0),
            ALTERNATE_FAMILY => (-160, 0, 0, 0),
            _ => (-210, -20, 20, 20),
        };
        assert_eq!(
            (
                state.distance,
                state.boost_response,
                state.brake_response,
                state.linked_target
            ),
            expected
        );
        assert_eq!(state.external_target, state.distance);
        assert_eq!(state.reserved_profile, [0; 2]);
        assert_eq!(state.pitch_height_offset, 0);
        assert!(state.capture_pending && state.switch_requested);
        let mut after = *f.records();
        after.view_distance = before.view_distance;
        assert_eq!(after, before);
    }
}

#[test]
fn missing_options_retains_capture_and_old_distance_filter_and_scene_prevents_retry() {
    let mut f = Fixture::new();
    let state = f.records().view_distance.as_mut().unwrap();
    state.capture_pending = true;
    state.distance = -40;
    f.world.player_view_options_enabled = None;
    assert_eq!(
        f.advance(),
        Err(SceneError::PlayerViewDistance(
            ViewDistanceError::MissingViewOptions
        ))
    );
    assert!(!f.records().view_distance.unwrap().capture_pending);
    assert!(f.capture().capture_position && f.capture().capture_rotation);
    assert!(f.objects.get(f.owner).unwrap().base.flags.view_side_filter);
    let objects = f.objects.clone();
    let records = *f.records();
    assert_eq!(f.advance(), Err(SceneError::Faulted));
    assert_eq!(f.objects, objects);
    assert_eq!(*f.records(), records);
}

#[test]
fn failed_transition_consumes_request_before_view_lookup_without_toggling_charge() {
    let mut f = Fixture::new();
    f.world.fixed_players[0] = None;
    f.world.scene.player_view_control = Some(0xAD);
    f.world.published_linked_view = Some(99);
    assert_eq!(
        f.advance(),
        Err(SceneError::PlayerViewDistance(
            ViewDistanceError::MissingFixedView
        ))
    );
    assert_eq!(f.world.scene.player_view_control, Some(0x8D));
    let records = *f.records();
    assert!(!records.view_distance.unwrap().switch_requested);
    assert!(!records.charge.unwrap().linked_mode);
    assert!(!records.charge.unwrap().linked_muzzle_disabled);
    assert_eq!(f.world.published_linked_view, Some(99));
    assert_eq!(records.view_distance.unwrap().distance, -240);
}

#[test]
fn real_transition_shares_charge_muzzle_flags_and_runs_the_actual_continuity_consumer() {
    let mut f = Fixture::new();
    f.world.scene.player_view_control = Some(0xAD);
    f.advance().unwrap();
    assert!(f.records().charge.unwrap().path_input().linked_mode);
    assert!(f.records().charge.unwrap().linked_muzzle_disabled);
    assert_eq!(f.world.published_linked_view, Some(1));
    assert_eq!(f.records().view_distance.unwrap().distance, -210);
    assert!(f.capture().capture_position && f.capture().capture_rotation);
    view_blend::advance(&mut f.objects, &f.world).unwrap();
    assert!(!f.capture().capture_position && !f.capture().capture_rotation);
    // Queue the return while still moving toward the linked view. It cannot
    // toggle until the previous transition's distance has reached tolerance.
    f.world.scene.player_view_control = Some(0x2D);
    for _ in 0..6 {
        f.advance().unwrap();
        view_blend::advance(&mut f.objects, &f.world).unwrap();
    }
    assert_eq!(f.records().view_distance.unwrap().distance, -30);
    assert!(f.records().view_distance.unwrap().switch_requested);
    assert!(!f.records().charge.unwrap().linked_muzzle_disabled);
    assert!(f.records().view_distance.unwrap().capture_pending);
    f.advance().unwrap();
    assert!(!f.records().charge.unwrap().linked_mode);
    assert!(f.records().charge.unwrap().linked_muzzle_disabled);
    assert_eq!(f.world.published_linked_view, Some(0));
    assert_eq!(f.records().view_distance.unwrap().distance, -60);
    // Side filtering sampled -30 before that move.
    assert!(f.objects.get(f.owner).unwrap().base.flags.view_side_filter);
}

#[test]
fn disabled_view_options_need_no_menu_input_and_ignored_contacts_choose_external_distance() {
    let mut f = Fixture::new();
    f.world.player_view_options_enabled = Some(false);
    f.world.scene.player_view_control = None;
    f.world.fixed_players[0] = None;
    f.records().charge.as_mut().unwrap().linked_mode = true;
    f.records().charge.as_mut().unwrap().linked_muzzle_disabled = true;
    f.records().contact.as_mut().unwrap().ignores_contacts = true;
    f.records().view_distance.as_mut().unwrap().distance = -120;
    f.advance().unwrap();
    assert_eq!(f.records().view_distance.unwrap().distance, -150);
    assert!(f.records().view_distance.unwrap().switch_requested);
    assert!(f.records().charge.unwrap().linked_mode);
    assert_eq!(f.world.published_linked_view, None);
}

#[test]
fn distance_chase_uses_wrapped_comparisons_and_settles_without_needing_fixed_view() {
    let mut f = Fixture::new();
    f.world.fixed_players[0] = None;
    f.records().view_distance.as_mut().unwrap().distance = 32760;
    f.records().view_distance.as_mut().unwrap().external_target = -32760;
    f.records().charge.as_mut().unwrap().linked_muzzle_disabled = true;
    f.advance().unwrap();
    assert_eq!(f.records().view_distance.unwrap().distance, -32760);
    assert!(f.records().view_distance.unwrap().capture_pending);
    assert!(!f.records().charge.unwrap().linked_muzzle_disabled);
    assert_eq!(
        f.advance(),
        Err(SceneError::PlayerViewDistance(
            ViewDistanceError::MissingFixedView
        ))
    );
    assert!(!f.records().view_distance.unwrap().capture_pending);
}
