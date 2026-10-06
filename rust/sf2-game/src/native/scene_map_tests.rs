use super::*;

const fn cursor(index: u16) -> MapCursor {
    MapCursor::from_index(index)
}

#[derive(Debug, Clone, Copy)]
enum Effect {
    Publish(u8),
    Health(u8),
    Fail,
}

#[derive(Default)]
struct Host {
    actors: ObjectStore,
    events: Vec<u8>,
    display_ready: bool,
    load_idle: bool,
    mode: u8,
    calls: usize,
}

impl SceneMapHost<Effect, MapActorSpawn> for Host {
    type Error = &'static str;
    fn condition(&self, condition: MapCondition) -> Result<bool, Self::Error> {
        match condition {
            MapCondition::DisplayReady => Ok(self.display_ready),
            MapCondition::LoadTableIdle => Ok(self.load_idle),
            MapCondition::ModeEquals(value) => Ok(self.mode == value),
            _ => Err("unprovided predicate"),
        }
    }
    fn apply(&mut self, effect: &Effect) -> Result<(), Self::Error> {
        self.calls += 1;
        match effect {
            Effect::Publish(value) => self.events.push(*value),
            Effect::Fail => return Err("service failed after entry"),
            Effect::Health(_) => return Err("health needs a selected actor"),
        }
        Ok(())
    }
    fn apply_to_current(&mut self, actor: ObjectId, effect: &Effect) -> Result<(), Self::Error> {
        self.calls += 1;
        match effect {
            Effect::Health(value) => {
                self.actors
                    .get_mut(actor)
                    .ok_or("stale actor")?
                    .base
                    .hit_points = *value;
                Ok(())
            }
            _ => Err("not an actor effect"),
        }
    }
    fn spawn(&mut self, specification: &MapActorSpawn) -> Result<Option<ObjectId>, Self::Error> {
        Ok(allocate_map_actor(
            &mut self.actors,
            ObjectSpawnDefaults::default(),
            *specification,
        ))
    }
}

fn spawn(x: i16) -> MapActorSpawn {
    MapActorSpawn {
        kind: ObjectKind::Effect,
        shape: ShapeId::EMPTY,
        behavior: Behavior::FollowPath,
        position: Vector3 { x, y: -125, z: 400 },
    }
}

type Instruction = MapInstruction<Effect, MapActorSpawn>;

fn visit(
    map: &mut SceneMap,
    catalog: &MapCatalog<'_, Effect, MapActorSpawn>,
    host: &mut Host,
) -> Result<MapReport, MapError<&'static str>> {
    map.visit(catalog, host, MapFramePolicy::default(), 32)
}

#[test]
fn phase_hold_is_a_per_visit_yield_and_release_preserves_its_last_marker() {
    let program: [Instruction; 3] = [
        Instruction::Yield {
            marker: PHASE_HOLD_MARKER,
            next: cursor(1),
        },
        Instruction::Jump(cursor(0)),
        Instruction::Stop,
    ];
    let exits = [PhaseExit {
        parked: cursor(1),
        continuation: cursor(2),
    }];
    let catalog = MapCatalog::new(&program, &exits).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    assert!(!map.release_phase(&catalog).unwrap());
    for update in 0..100 {
        let report = visit(&mut map, &catalog, &mut host).unwrap();
        assert_eq!(report.commands, if update == 0 { 1 } else { 2 });
        assert_eq!(report.stop, MapStop::Yielded(PHASE_HOLD_MARKER));
        assert_eq!(map.cursor(), cursor(1));
        assert_eq!(map.yield_marker(), PHASE_HOLD_MARKER);
    }
    assert!(map.release_phase(&catalog).unwrap());
    assert_eq!(map.yield_marker(), PHASE_HOLD_MARKER);
    assert_eq!(
        visit(&mut map, &catalog, &mut host).unwrap().stop,
        MapStop::Stopped
    );
    assert!(!map.release_phase(&catalog).unwrap());
}

#[test]
fn zero_delay_falls_through_without_erasing_a_preceding_marker() {
    let program: [Instruction; 4] = [
        Instruction::Yield {
            marker: 99,
            next: cursor(1),
        },
        Instruction::Yield {
            marker: 0,
            next: cursor(2),
        },
        Instruction::Apply {
            effect: Effect::Publish(7),
            next: cursor(3),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    assert_eq!(
        visit(&mut map, &catalog, &mut host).unwrap().stop,
        MapStop::Yielded(99)
    );
    assert_eq!(visit(&mut map, &catalog, &mut host).unwrap().commands, 3);
    assert_eq!(map.yield_marker(), 99);
    assert_eq!(host.events, [7]);
}

#[test]
fn display_and_load_waits_retry_without_repeating_prior_effects() {
    let program: [Instruction; 5] = [
        Instruction::Apply {
            effect: Effect::Publish(1),
            next: cursor(1),
        },
        Instruction::Await {
            condition: MapCondition::DisplayReady,
            retry_marker: Some(1),
            next: cursor(2),
        },
        Instruction::Apply {
            effect: Effect::Publish(2),
            next: cursor(3),
        },
        Instruction::Await {
            condition: MapCondition::LoadTableIdle,
            retry_marker: None,
            next: cursor(4),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    for _ in 0..3 {
        assert_eq!(
            visit(&mut map, &catalog, &mut host).unwrap().stop,
            MapStop::Waiting(MapCondition::DisplayReady)
        );
        assert_eq!(host.events, [1]);
        assert_eq!(map.yield_marker(), 1);
    }
    host.display_ready = true;
    for _ in 0..3 {
        assert_eq!(
            visit(&mut map, &catalog, &mut host).unwrap().stop,
            MapStop::Waiting(MapCondition::LoadTableIdle)
        );
        assert_eq!(host.events, [1, 2]);
        assert_eq!(map.yield_marker(), 1);
    }
    host.load_idle = true;
    assert_eq!(
        visit(&mut map, &catalog, &mut host).unwrap().stop,
        MapStop::Stopped
    );
}

#[test]
fn each_mode_byte_selects_only_its_source_branch() {
    let program: [Instruction; 4] = [
        Instruction::Branch {
            condition: MapCondition::ModeEquals(4),
            taken: cursor(1),
            otherwise: cursor(2),
        },
        Instruction::Apply {
            effect: Effect::Publish(1),
            next: cursor(3),
        },
        Instruction::Apply {
            effect: Effect::Publish(2),
            next: cursor(3),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    for mode in 0..=u8::MAX {
        let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
        let mut host = Host {
            mode,
            ..Default::default()
        };
        assert_eq!(visit(&mut map, &catalog, &mut host).unwrap().commands, 3);
        assert_eq!(host.events, [if mode == 4 { 1 } else { 2 }]);
    }
}

#[test]
fn map_spawns_insert_after_the_list_head_and_delay_initial_strategy_visits() {
    let program = [
        Instruction::Spawn {
            specification: spawn(1),
            marker: 0,
            next: cursor(1),
        },
        Instruction::Spawn {
            specification: spawn(2),
            marker: 0,
            next: cursor(2),
        },
        Instruction::Spawn {
            specification: spawn(3),
            marker: 0,
            next: cursor(3),
        },
        Instruction::ApplyToCurrent {
            effect: Effect::Health(100),
            next: cursor(4),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    visit(&mut map, &catalog, &mut host).unwrap();
    assert_eq!(
        host.actors
            .active_ids()
            .iter()
            .map(|id| id.index())
            .collect::<Vec<_>>(),
        [0, 2, 1]
    );
    assert_eq!(map.current_object().unwrap().index(), 2);
    for (id, actor) in host.actors.active_objects() {
        assert_eq!(actor.base.position.x, id.index() as i16 + 1);
        assert_eq!(actor.base.speed, 0);
        assert!(actor.base.contacts.first_strategy_visit);
        assert!(actor.extension.path_state.hold_latched);
        assert_eq!(actor.base.hit_points, if id.index() == 2 { 100 } else { 0 });
    }
}

#[test]
fn allocation_failure_clears_selection_and_still_publishes_spawn_marker() {
    let program = [
        Instruction::Spawn {
            specification: spawn(1),
            marker: 73,
            next: cursor(1),
        },
        Instruction::Spawn {
            specification: spawn(2),
            marker: 0,
            next: cursor(2),
        },
        Instruction::ApplyToCurrent {
            effect: Effect::Health(100),
            next: cursor(3),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    assert_eq!(
        visit(&mut map, &catalog, &mut host).unwrap().stop,
        MapStop::Yielded(73)
    );
    assert!(map.current_object().is_some());
    while allocate_map_actor(&mut host.actors, ObjectSpawnDefaults::default(), spawn(0)).is_some() {
    }
    assert_eq!(
        visit(&mut map, &catalog, &mut host).unwrap().stop,
        MapStop::Stopped
    );
    assert_eq!(map.current_object(), None);
    assert_eq!(map.yield_marker(), 0);
    assert_eq!(
        host.calls, 0,
        "a null current object does not invoke its writer"
    );
    assert!(host
        .actors
        .active_objects()
        .all(|(_, actor)| actor.base.hit_points == 0));
}

#[test]
fn spawn_samples_live_group_and_pause_defaults_without_reusing_old_actor_state() {
    let mut objects = ObjectStore::new();
    for (group, run_when_paused) in [(0, false), (255, true), (4, false)] {
        let actor = allocate_map_actor(
            &mut objects,
            ObjectSpawnDefaults {
                group,
                run_when_paused,
            },
            spawn(1),
        )
        .unwrap();
        let actor = objects.get(actor).unwrap();
        assert_eq!(actor.extension.spawn_group, group);
        assert_eq!(actor.base.contacts.run_when_paused, run_when_paused);
        assert_eq!(actor.base.attack_power, 0);
        assert_eq!(actor.base.target_speed, 0);
    }
}

#[test]
fn consuming_the_last_map_slot_does_not_mark_existing_effects_for_retirement() {
    let mut objects = ObjectStore::new();
    let mut specification = spawn(1);
    specification.shape = ShapeId::from_catalog_index(9);
    let effect =
        allocate_map_actor(&mut objects, ObjectSpawnDefaults::default(), specification).unwrap();
    objects
        .get_mut(effect)
        .unwrap()
        .base
        .flags
        .reclaim_on_pool_pressure = true;
    while allocate_map_actor(&mut objects, ObjectSpawnDefaults::default(), spawn(0)).is_some() {}
    assert!(!objects.get(effect).unwrap().base.flags.remove_after_tick);
}

#[test]
fn failed_service_latches_the_owner_after_earlier_effects_and_cannot_be_replayed() {
    let program: [Instruction; 3] = [
        Instruction::Apply {
            effect: Effect::Publish(9),
            next: cursor(1),
        },
        Instruction::Apply {
            effect: Effect::Fail,
            next: cursor(2),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    assert_eq!(
        visit(&mut map, &catalog, &mut host),
        Err(MapError::Host("service failed after entry"))
    );
    assert_eq!(map.cursor(), cursor(1));
    assert!(map.is_faulted());
    assert_eq!(host.events, [9]);
    assert_eq!(visit(&mut map, &catalog, &mut host), Err(MapError::Faulted));
    assert_eq!(map.redirect(&catalog, cursor(0)), Err(MapError::Faulted));
    assert_eq!(map.release_phase(&catalog), Err(MapError::Faulted));
    assert_eq!(host.calls, 2);
}

#[test]
fn a_missing_predicate_faults_before_any_branch_effect() {
    let program: [Instruction; 2] = [
        Instruction::Branch {
            condition: MapCondition::ExternalEvent,
            taken: cursor(1),
            otherwise: cursor(1),
        },
        Instruction::Stop,
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    assert_eq!(
        visit(&mut map, &catalog, &mut host),
        Err(MapError::Host("unprovided predicate"))
    );
    assert_eq!(host.calls, 0);
    assert_eq!(map.cursor(), cursor(0));
}

#[test]
fn exhausted_instruction_budget_is_a_fault_not_a_gameplay_yield() {
    let program: [Instruction; 2] = [
        Instruction::Apply {
            effect: Effect::Publish(1),
            next: cursor(1),
        },
        Instruction::Jump(cursor(0)),
    ];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
    let mut host = Host::default();
    assert_eq!(
        map.visit(&catalog, &mut host, MapFramePolicy::default(), 4),
        Err(MapError::BudgetExhausted)
    );
    assert_eq!(host.events, [1, 1]);
    assert_eq!(visit(&mut map, &catalog, &mut host), Err(MapError::Faulted));
    assert_eq!(host.events, [1, 1]);
}

#[test]
fn suppression_preserves_all_state_without_querying_services() {
    let program: [Instruction; 1] = [Instruction::Stop];
    let catalog = MapCatalog::new(&program, &[]).unwrap();
    let mut host = Host::default();
    for alternate_view in [false, true] {
        for suppress_alternate_map in [false, true] {
            let mut map = SceneMap::new(&catalog, cursor(0)).unwrap();
            let before = map.clone();
            let result = map
                .visit(
                    &catalog,
                    &mut host,
                    MapFramePolicy {
                        alternate_view,
                        suppress_alternate_map,
                    },
                    1,
                )
                .unwrap();
            assert_eq!(
                result.stop,
                if alternate_view && suppress_alternate_map {
                    MapStop::Suppressed
                } else {
                    MapStop::Stopped
                }
            );
            assert_eq!(
                result.commands,
                usize::from(!(alternate_view && suppress_alternate_map))
            );
            assert_eq!(map, before);
        }
    }
}

#[test]
fn catalog_rejects_bad_targets_and_unproven_phase_exits_before_execution() {
    let bad: [Instruction; 1] = [Instruction::Jump(cursor(1))];
    assert!(
        matches!(MapCatalog::new(&bad, &[]), Err(CatalogError::InvalidCursor(value)) if value == cursor(1))
    );
    let stop: [Instruction; 1] = [Instruction::Stop];
    let exit = [PhaseExit {
        parked: cursor(0),
        continuation: cursor(0),
    }];
    assert!(matches!(
        MapCatalog::new(&stop, &exit),
        Err(CatalogError::InvalidPhaseExit(_))
    ));
    let looped: [Instruction; 2] = [
        Instruction::Yield {
            marker: PHASE_HOLD_MARKER,
            next: cursor(1),
        },
        Instruction::Jump(cursor(0)),
    ];
    let duplicates = [PhaseExit {
        parked: cursor(1),
        continuation: cursor(0),
    }; 2];
    assert!(matches!(
        MapCatalog::new(&looped, &duplicates),
        Err(CatalogError::DuplicatePhaseExit(_))
    ));
}
