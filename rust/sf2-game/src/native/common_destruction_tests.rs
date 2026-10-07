use super::*;
use crate::collision_contacts::{Contact, ContactHost, ContactId, ContactStore};
use crate::path_appearance::AnimationChannels;
use crate::path_runtime::PathRuntime;
use crate::strategy_schedule::{
    select_strategy, StrategyAction, StrategyCompletion, StrategyHost, StrategyInputs,
    StrategySchedule,
};
use crate::{Angle, PathCursor, PathId};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Override(ObjectId),
    Map(ObjectId, MapDeathCounts),
    Sound(AuthoredCue),
    Strategy(ObjectId, u16),
    Separate(ObjectId),
    ReleasePrograms(ObjectId),
}

struct World {
    objects: ObjectStore,
    proxies: SceneProxyStore,
    paths: PathRuntime,
    contacts: ContactStore,
    counts: MapDeathCounts,
    events: Vec<Event>,
    inputs: Option<EffectInputs>,
    override_returns: Option<ObjectId>,
    map_suppression: Option<bool>,
    fail_map: bool,
}

impl World {
    fn new() -> Self {
        Self {
            objects: ObjectStore::new(),
            proxies: SceneProxyStore::default(),
            paths: PathRuntime::default(),
            contacts: ContactStore::default(),
            counts: MapDeathCounts {
                active: 0xAB00,
                destroyed: 0xCDFF,
            },
            events: Vec::new(),
            inputs: Some(EffectInputs {
                spawn: ObjectSpawnDefaults {
                    group: 91,
                    run_when_paused: false,
                },
                primary_marker: Vector3::default(),
                secondary_marker: Some(Vector3 {
                    x: 2000,
                    y: 32767,
                    z: 0,
                }),
            }),
            override_returns: None,
            map_suppression: None,
            fail_map: false,
        }
    }

    fn actor(&mut self, shape: u16) -> ObjectId {
        let mut actor = Object::new_authored(
            ObjectKind::Enemy,
            ShapeId::from_catalog_index(shape),
            Behavior::FollowPath,
            ObjectSpawnDefaults::default(),
        );
        actor.base.position = Vector3 {
            x: 23,
            y: -400,
            z: 17,
        };
        actor.base.hit_points = 0;
        actor.base.contacts.first_strategy_visit = false;
        self.objects.allocate(actor).unwrap()
    }

    fn fill(&mut self, free: usize) {
        while self.objects.len() < OBJECT_CAPACITY - free {
            self.actor(89);
        }
    }
}

impl DestructionHost for World {
    type Error = &'static str;
    fn objects(&self) -> &ObjectStore {
        &self.objects
    }
    fn objects_mut(&mut self) -> &mut ObjectStore {
        &mut self.objects
    }
    fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore) {
        (&mut self.objects, &mut self.proxies)
    }
    fn run_death_override(&mut self, owner: ObjectId) -> Result<Option<ObjectId>, Self::Error> {
        self.events.push(Event::Override(owner));
        if let Some(returned) = self.override_returns {
            self.objects.get_mut(returned).unwrap().base.hit_points = 11;
        }
        Ok(self.override_returns)
    }
    fn map_death_counts(&mut self) -> Result<&mut MapDeathCounts, Self::Error> {
        Ok(&mut self.counts)
    }
    fn resume_map_on_death(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
        self.events.push(Event::Map(owner, self.counts));
        assert!(self
            .objects
            .get(owner)
            .unwrap()
            .extension
            .scene_proxy
            .is_some());
        if self.fail_map {
            return Err("map callback failed");
        }
        if let Some(value) = self.map_suppression {
            self.objects
                .get_mut(owner)
                .unwrap()
                .base
                .flags
                .suppress_death_effects = value;
        }
        Ok(())
    }
    fn effect_inputs(&mut self) -> Result<EffectInputs, Self::Error> {
        self.inputs.ok_or("missing view markers")
    }
    fn queue_death_sound(&mut self, cue: AuthoredCue) -> Result<(), Self::Error> {
        self.events.push(Event::Sound(cue));
        Ok(())
    }
}

fn cursor() -> PathCursor {
    PathCursor {
        path: PathId::from_catalog_index(0),
        command_index: 0,
    }
}

fn proxy(world: &mut World, owner: ObjectId) -> super::super::scene_proxy::SceneProxyId {
    world
        .proxies
        .capture_actor(&mut world.objects, owner, cursor(), &world.paths.resources)
        .unwrap()
        .unwrap()
}

#[test]
fn override_tail_dispatch_bypasses_counts_sounds_effects_and_cleanup() {
    let mut world = World::new();
    let owner = world.actor(64);
    let returned = world.actor(89);
    world
        .objects
        .get_mut(owner)
        .unwrap()
        .base
        .flags
        .tracked_map_actor = true;
    let saved_proxy = proxy(&mut world, owner);
    world.override_returns = Some(returned);
    world.inputs = None;
    let before = world.objects.get(owner).unwrap().clone();
    assert_eq!(destroy(&mut world, owner), Ok(returned));
    assert_eq!(world.objects.get(owner), Some(&before));
    assert_eq!(world.objects.get(returned).unwrap().base.hit_points, 11);
    assert_eq!(
        world.counts,
        MapDeathCounts {
            active: 0xAB00,
            destroyed: 0xCDFF
        }
    );
    assert!(world.proxies.get(saved_proxy).is_some());
    assert_eq!(world.events, [Event::Override(owner)]);
}

#[test]
fn map_callback_observes_wrapped_low_counts_and_live_proxy_before_effect_gate() {
    for initial_gate in [false, true] {
        let mut world = World::new();
        let owner = world.actor(64);
        let actor = world.objects.get_mut(owner).unwrap();
        actor.base.flags.tracked_map_actor = true;
        actor.base.flags.suppress_death_effects = initial_gate;
        let saved_proxy = proxy(&mut world, owner);
        world.map_suppression = Some(!initial_gate);
        if !initial_gate {
            world.inputs = None;
        }
        assert_eq!(destroy(&mut world, owner), Ok(owner));
        assert_eq!(
            &world.events[..2],
            [
                Event::Override(owner),
                Event::Map(
                    owner,
                    MapDeathCounts {
                        active: 0xABFF,
                        destroyed: 0xCD00
                    }
                )
            ]
        );
        assert_eq!(world.objects.len(), if initial_gate { 3 } else { 1 });
        assert_eq!(world.events.len(), if initial_gate { 4 } else { 2 });
        assert!(world.proxies.get(saved_proxy).is_none());
        assert!(
            world
                .objects
                .get(owner)
                .unwrap()
                .base
                .flags
                .remove_after_tick
        );
    }
}

#[test]
fn every_map_low_byte_wraps_without_borrow_or_carry_into_its_high_byte() {
    for high in 0..=u8::MAX {
        for low in 0..=u8::MAX {
            let mut counts = MapDeathCounts {
                active: u16::from(high) * 256 + u16::from(low),
                destroyed: u16::from(255 - high) * 256 + u16::from(low),
            };
            counts.record_death();
            assert_eq!(
                counts.active,
                u16::from(high) * 256 + u16::from(low.wrapping_sub(1))
            );
            assert_eq!(
                counts.destroyed,
                u16::from(255 - high) * 256 + u16::from(low.wrapping_add(1))
            );
        }
    }
}

#[test]
fn default_effects_use_global_head_order_fresh_fields_and_exact_shape_profiles() {
    // Header 0: diameter 0, shift 0. Header 89: diameter 100, shift 0.
    // Header 64: diameter 672, shift 3. These are independently decoded data.
    for (shape, effect_shape, bias, duration, children) in
        [(0, 9, 248, 4, 1), (89, 10, 254, 6, 1), (64, 12, 0, 8, 2)]
    {
        let mut world = World::new();
        let owner = world.actor(shape);
        let head = world.actor(89);
        let before = world.objects.get(owner).unwrap().clone();
        assert_eq!(destroy(&mut world, owner), Ok(owner));
        let ids = world.objects.active_ids();
        assert_eq!(ids[0], head);
        assert_eq!(*ids.last().unwrap(), owner);
        assert_eq!(world.objects.len(), 2 + children);
        // Sprite allocates first; a huge companion inserts ahead of it.
        let sprite = world.objects.get(ids[children]).unwrap();
        let mut expected = fresh_effect(ObjectSpawnDefaults {
            group: 91,
            run_when_paused: false,
        });
        expected.base.shape = ShapeId::from_catalog_index(effect_shape);
        expected.base.flags.scaled_sprite = true;
        expected.base.hit_points = 1;
        expected.base.position = before.base.position;
        expected.base.acceleration = duration;
        expected.extension.texture_scroll_x = bias;
        expected.extension.path_state.animation.color = AnimationControl::from_packed(0x80);
        expected.base.previous = sprite.base.previous;
        expected.base.next = sprite.base.next;
        assert_eq!(sprite, &expected);
        let dead = world.objects.get(owner).unwrap();
        assert_eq!(dead.base.shape, ShapeId::EMPTY);
        assert_eq!(
            dead.base.behavior,
            Behavior::Destruction(EffectPhase::Animate)
        );
        assert!(dead.base.flags.remove_after_tick);
        assert_eq!(dead.base.hit_points, 0);
        if children == 2 {
            let companion = world.objects.get(ids[1]).unwrap();
            assert_eq!(companion.base.shape, ShapeId::EMPTY);
            assert_eq!(companion.base.hit_points, 0);
            assert!(companion.base.contacts.first_strategy_visit);
            assert_eq!(
                (
                    companion.base.child_number,
                    companion.base.wait_timer,
                    companion.extension.path_state.repeat_counter
                ),
                (30, 7, 30)
            );
            assert_eq!(companion.base.acceleration, 2);
            assert_eq!(
                companion.extension.path_state.animation,
                AnimationChannels::default()
            );
            assert!(companion.base.flags.reclaim_on_pool_pressure);
            assert!(!companion.base.flags.scaled_sprite);
        }
        assert_eq!(
            world.events,
            [
                Event::Override(owner),
                Event::Sound(AuthoredCue::new(112, 0x30, PlayerTarget::Primary)),
                Event::Sound(AuthoredCue::new(112, 0, PlayerTarget::Primary))
            ]
        );
    }
}

#[test]
fn disabled_secondary_view_omits_its_cue_without_changing_primary_routing() {
    let mut world = World::new();
    let owner = world.actor(0);
    world.inputs.as_mut().unwrap().secondary_marker = None;
    world.inputs.as_mut().unwrap().primary_marker.x = 5000;
    destroy(&mut world, owner).unwrap();
    assert_eq!(
        world.events,
        [
            Event::Override(owner),
            Event::Sound(AuthoredCue::new(112, 0x60, PlayerTarget::Primary))
        ]
    );
}

#[test]
fn resource_diagnostics_retain_only_source_ordered_prefix_side_effects() {
    for free in [0, 1] {
        let mut world = World::new();
        let owner = world.actor(64);
        let saved_proxy = proxy(&mut world, owner);
        world.fill(free);
        assert_eq!(
            destroy(&mut world, owner),
            Err(DestructionError::ObjectCapacity)
        );
        assert_eq!(world.objects.len(), OBJECT_CAPACITY);
        assert_eq!(world.events.len(), 3); // Both sounds precede allocation.
        let actor = world.objects.get(owner).unwrap();
        assert_eq!(actor.base.shape, ShapeId::from_catalog_index(64));
        assert_eq!(actor.base.behavior, Behavior::FollowPath);
        assert!(!actor.base.flags.remove_after_tick);
        assert!(actor.base.contacts.run_when_paused);
        assert!(actor.base.flags.exclude_from_shape_footprint_search);
        assert!(world.proxies.get(saved_proxy).is_some());
        if free == 1 {
            let prefix_sprite = world.objects.get(world.objects.active_ids()[1]).unwrap();
            assert_eq!(prefix_sprite.base.shape, ShapeId::EMPTY);
            assert_eq!(prefix_sprite.base.hit_points, 0);
            assert!(prefix_sprite.base.flags.scaled_sprite);
        }
    }
    let mut world = World::new();
    let owner = world.actor(64);
    let saved_proxy = proxy(&mut world, owner);
    world
        .objects
        .get_mut(owner)
        .unwrap()
        .base
        .flags
        .tracked_map_actor = true;
    world.fail_map = true;
    assert_eq!(
        destroy(&mut world, owner),
        Err(DestructionError::Host("map callback failed"))
    );
    assert_eq!(world.counts.active, 0xABFF);
    assert_eq!(world.events.len(), 2);
    assert!(world.proxies.get(saved_proxy).is_some());
    assert!(
        !world
            .objects
            .get(owner)
            .unwrap()
            .base
            .flags
            .remove_after_tick
    );
}

#[test]
fn death_tail_kills_owned_children_without_retiring_contacts_links_or_programs() {
    let mut world = World::new();
    let owner = world.actor(64);
    let owned = world.actor(89);
    let independent = world.actor(89);
    let grandchild = world.actor(89);
    for (parent, child, number) in [
        (owner, owned, 1),
        (owner, independent, 2),
        (owned, grandchild, 3),
    ] {
        crate::path_relationships::attach_fresh_child(&mut world.objects, parent, child, number)
            .unwrap();
        let actor = world.objects.get_mut(child).unwrap();
        actor.extension.parent = Some(parent);
        actor.base.hit_points = 77;
    }
    world
        .objects
        .get_mut(independent)
        .unwrap()
        .base
        .flags
        .remove_with_parent = false;
    world
        .objects
        .get_mut(independent)
        .unwrap()
        .base
        .linked_object = Some(owner);
    world
        .objects
        .get_mut(owner)
        .unwrap()
        .base
        .flags
        .suppress_death_effects = true;
    world.objects.get_mut(owner).unwrap().base.path = Some(cursor());
    world
        .paths
        .call(&mut world.objects, owner, cursor(), cursor())
        .unwrap();
    let resources = world.paths.resources.clone();
    let pair = world
        .contacts
        .record_pair(owner, independent, [Some(7), Some(3)])
        .unwrap();
    let contacts_before = pair.map(|id| *world.contacts.get(id).unwrap());
    let active = world.objects.active_ids().to_vec();
    let saved_proxy = proxy(&mut world, owner);
    let mut grand_expected = world.objects.get(grandchild).unwrap().clone();
    grand_expected.extension.path_state.motion.attached_coordinates = false;
    grand_expected.base.attachment = None;
    grand_expected.base.hit_points = 0;
    grand_expected.base.flags.collision_disabled = true;
    world.inputs = None;
    destroy(&mut world, owner).unwrap();
    assert_eq!(world.objects.active_ids(), active);
    assert_eq!(world.paths.resources, resources);
    assert_eq!(
        pair.map(|id| *world.contacts.get(id).unwrap()),
        contacts_before
    );
    assert!(world.proxies.get(saved_proxy).is_none());
    assert_eq!(world.objects.get(grandchild), Some(&grand_expected));
    for child in [owned, independent] {
        let actor = world.objects.get(child).unwrap();
        assert_eq!(actor.base.attachment, None);
        assert_eq!(actor.extension.parent, Some(owner));
        assert!(!actor.extension.path_state.motion.attached_coordinates);
        assert!(!actor.base.flags.remove_after_tick);
    }
    assert_eq!(world.objects.get(owned).unwrap().base.hit_points, 0);
    assert!(
        world
            .objects
            .get(owned)
            .unwrap()
            .base
            .flags
            .collision_disabled
    );
    assert_eq!(world.objects.get(independent).unwrap().base.hit_points, 77);
    assert_eq!(
        world.objects.get(independent).unwrap().base.linked_object,
        Some(owner)
    );
    assert_eq!(
        world.objects.get(owner).unwrap().base.attachment_next,
        Some(owned)
    );
    assert_eq!(
        world.objects.get(owned).unwrap().base.attachment_next,
        Some(independent)
    );
    assert!(
        !world
            .objects
            .get(owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .refresh_child_chain
    );
}

#[test]
fn child_gate_and_invalid_chain_preserve_unrelated_state() {
    let mut world = World::new();
    let owner = world.actor(0);
    world.objects.get_mut(owner).unwrap().base.attachment_next = Some(owner);
    let before = world.objects.clone();
    detach_dying_children(&mut world.objects, owner).unwrap();
    assert_eq!(world.objects, before); // Gate skips even a stale chain.
    world
        .objects
        .get_mut(owner)
        .unwrap()
        .extension
        .path_state
        .motion
        .refresh_child_chain = true;
    let before = world.objects.clone();
    assert_eq!(
        detach_dying_children(&mut world.objects, owner),
        Err(RelationshipError::ChildCycle(owner))
    );
    assert_eq!(world.objects, before);
}

#[test]
fn effect_animation_uses_wrapping_signed_comparison_and_exact_color_control() {
    let motion = EffectMotion {
        primary_mode: 0,
        displacement: Vector3::default(),
    };
    for age in 0..=u8::MAX {
        for duration in 0..=u8::MAX {
            let mut actor = fresh_effect(ObjectSpawnDefaults::default());
            actor.base.target_speed = age;
            actor.base.acceleration = duration;
            actor.extension.path_state.animation.color = AnimationControl::from_packed(age);
            let expected_age = age.wrapping_add(1);
            let animates = ((u16::from(expected_age) + 256 - u16::from(duration)) & 0x80) != 0;
            step_effect(&mut actor, Some(motion)).unwrap();
            assert_eq!(actor.base.target_speed, expected_age);
            assert_eq!(actor.base.flags.remove_after_tick, !animates);
            assert_eq!(actor.base.flags.visible, animates);
            if animates {
                let mut value = age.wrapping_add(1);
                if value < 128 {
                    value = value.wrapping_add(8);
                }
                value &= 127;
                if value >= 8 {
                    value -= 8;
                }
                assert_eq!(
                    actor.extension.path_state.animation.color.packed(),
                    value | 128
                );
                assert_eq!(actor.extension.color_frame, value);
            } else {
                assert_eq!(actor.extension.path_state.animation.color.packed(), age);
            }
        }
    }
}

#[test]
fn effect_motion_uses_primary_high_nibble_only_and_trail_needs_no_player_input() {
    for mode in 0..=u8::MAX {
        let mut actor = fresh_effect(ObjectSpawnDefaults::default());
        actor.base.acceleration = 8;
        actor.base.position = Vector3 {
            x: 32767,
            y: 400,
            z: -32768,
        };
        actor.base.velocity = Vector3 {
            x: 200,
            y: -300,
            z: 400,
        };
        actor.base.yaw = Angle::from_units(97);
        let before = actor.clone();
        assert_eq!(
            step_effect(&mut actor, None),
            Err(EffectError::MissingMotion)
        );
        assert_eq!(actor, before);
        step_effect(
            &mut actor,
            Some(EffectMotion {
                primary_mode: mode,
                displacement: Vector3 {
                    x: -32768,
                    y: 32767,
                    z: 32767,
                },
            }),
        )
        .unwrap();
        assert_eq!(
            actor.base.position,
            Vector3 {
                x: 32767,
                y: 400,
                z: if mode & 240 == 16 { -32766 } else { -32768 }
            }
        );
        assert_eq!(actor.base.velocity, before.base.velocity);
        assert_eq!(actor.base.yaw, before.base.yaw);
    }
    let mut actor = fresh_effect(ObjectSpawnDefaults::default());
    actor.base.flags.reclaim_on_pool_pressure = true;
    actor.base.flags.casts_shadow = true;
    actor.base.hit_points = 1; // Authored health change makes trail reachable.
    actor.base.acceleration = 1;
    actor.base.child_number = 30;
    actor.base.wait_timer = 7;
    actor.extension.path_state.repeat_counter = 30;
    step_effect(
        &mut actor,
        Some(EffectMotion {
            primary_mode: 0,
            displacement: Vector3::default(),
        }),
    )
    .unwrap();
    assert_eq!(
        actor.base.behavior,
        Behavior::Destruction(EffectPhase::Trail)
    );
    assert_eq!(actor.base.target_speed, 63); // Same-invocation decrement.
    assert_eq!(
        (
            actor.base.child_number,
            actor.base.wait_timer,
            actor.extension.path_state.repeat_counter
        ),
        (0, 0, 0)
    );
    assert!(!actor.base.flags.casts_shadow);
    for remaining in (0..63).rev() {
        step_effect(&mut actor, None).unwrap();
        assert_eq!(actor.base.target_speed, remaining);
        assert!(!actor.base.flags.remove_after_tick);
    }
    step_effect(&mut actor, None).unwrap();
    assert!(!actor.base.flags.visible);
    assert!(actor.base.flags.remove_after_tick);
}

impl StrategyHost for World {
    type Error = DestructionError<&'static str>;
    fn objects(&self) -> &ObjectStore {
        &self.objects
    }
    fn strategy_suspended(&self, _: ObjectId) -> bool {
        false
    }
    fn run_strategy(
        &mut self,
        owner: ObjectId,
        clock: u16,
    ) -> Result<StrategyCompletion, Self::Error> {
        self.events.push(Event::Strategy(owner, clock));
        let actor = self.objects.get(owner).unwrap();
        let decision = select_strategy(
            StrategyInputs {
                health: actor.base.hit_points,
                suspended: actor.base.flags.strategy_suspended,
                excluded_actor: false,
                first_visit: actor.base.contacts.first_strategy_visit,
                hit_pending: actor.base.contacts.pending_hit,
                run_when_paused: actor.base.contacts.run_when_paused,
                has_assigned_strategy: true,
            },
            false,
        );
        if decision.clear_first_visit {
            self.objects
                .get_mut(owner)
                .unwrap()
                .base
                .contacts
                .first_strategy_visit = false;
        }
        match decision.action {
            StrategyAction::CommonDestruction => {
                destroy(self, owner)?;
            }
            StrategyAction::Assigned => {
                step_effect(
                    self.objects.get_mut(owner).unwrap(),
                    Some(EffectMotion {
                        primary_mode: 0,
                        displacement: Vector3::default(),
                    }),
                )
                .expect("assigned death effect");
            }
            _ => panic!("unexpected strategy in death-effect scene"),
        }
        Ok(StrategyCompletion::keep(owner))
    }
    fn retire_object(&mut self, _: ObjectId) -> Result<(), Self::Error> {
        panic!("death effects request deferred cleanup, never immediate retirement")
    }
}

#[test]
fn live_scheduler_visits_births_in_source_order_and_companion_dies_next_epoch() {
    for overlap_visits in 0_usize..=4 {
        let mut world = World::new();
        let owner = world.actor(64);
        let mut schedule = StrategySchedule::default();
        schedule
            .begin::<DestructionError<&str>>(&world.objects)
            .unwrap();
        let mut remaining = overlap_visits;
        schedule
            .run_overlapping(&mut world, || {
                let pending = remaining > 0;
                remaining = remaining.saturating_sub(1);
                pending
            })
            .unwrap();
        schedule.run_remainder(&mut world).unwrap();
        let ids = world.objects.active_ids().to_vec();
        assert_eq!(ids[0], owner);
        let companion = ids[1];
        let sprite = ids[2];
        assert_eq!(world.objects.get(companion).unwrap().base.target_speed, 1);
        assert_eq!(world.objects.get(sprite).unwrap().base.target_speed, 1);
        assert!(
            !world
                .objects
                .get(companion)
                .unwrap()
                .base
                .contacts
                .first_strategy_visit
        );
        crate::retirement::retire(&mut world, owner).unwrap();
        world.events.clear();
        schedule
            .begin::<DestructionError<&str>>(&world.objects)
            .unwrap();
        schedule.run_overlapping(&mut world, || true).unwrap();
        schedule.run_remainder(&mut world).unwrap();
        let visits: Vec<_> = world
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Strategy(id, clock) => Some((*id, *clock)),
                _ => None,
            })
            .collect();
        assert_eq!(visits, [(companion, 2), (owner, 2), (sprite, 2)]);
        let child_sprite = world.objects.get(owner).unwrap();
        assert_eq!(child_sprite.base.shape, ShapeId::from_catalog_index(9));
        assert_eq!(child_sprite.base.target_speed, 1);
        assert_eq!(child_sprite.extension.texture_scroll_x, 248);
        assert!(
            world
                .objects
                .get(companion)
                .unwrap()
                .base
                .flags
                .remove_after_tick
        );
        assert_eq!(world.objects.get(sprite).unwrap().base.target_speed, 2);
        assert_eq!(schedule.clock(), 2);
    }
}

impl ContactHost for World {
    type Error = &'static str;
    fn contacts(&self) -> &ContactStore {
        &self.contacts
    }
    fn contacts_mut(&mut self) -> &mut ContactStore {
        &mut self.contacts
    }
    fn on_separation(&mut self, _: ContactId, contact: Contact) -> Result<(), Self::Error> {
        assert!(self.objects.get(contact.owner).is_some());
        assert!(self.objects.get(contact.other).is_some());
        self.events.push(Event::Separate(contact.owner));
        Ok(())
    }
}

impl crate::retirement::RetirementHost for World {
    fn objects(&self) -> &ObjectStore {
        &self.objects
    }
    fn objects_and_proxies_mut(&mut self) -> (&mut ObjectStore, &mut SceneProxyStore) {
        (&mut self.objects, &mut self.proxies)
    }
    fn release_actor_programs(&mut self, owner: ObjectId) -> Result<(), Self::Error> {
        self.events.push(Event::ReleasePrograms(owner));
        self.paths
            .release_actor_programs(&mut self.objects, owner)
            .map_err(|_| "program release failed")
    }
}

#[test]
fn death_then_full_retirement_keeps_contact_callbacks_and_program_release_in_order() {
    let mut world = World::new();
    let owner = world.actor(0);
    let other = world.actor(89);
    world.objects.get_mut(owner).unwrap().base.path = Some(cursor());
    world
        .paths
        .call(&mut world.objects, owner, cursor(), cursor())
        .unwrap();
    world
        .contacts
        .record_pair(owner, other, [None, None])
        .unwrap();
    proxy(&mut world, owner);
    destroy(&mut world, owner).unwrap();
    assert!(world.paths.resources.owner_count(owner) > 0);
    assert_eq!(world.contacts.len(), 2);
    assert_eq!(world.events.len(), 3);
    let lifetime = world.objects.lifetime_id(owner).unwrap();
    world.events.clear();
    crate::retirement::retire(&mut world, owner).unwrap();
    assert_eq!(
        world.events,
        [
            Event::Separate(owner),
            Event::Separate(other),
            Event::ReleasePrograms(owner)
        ]
    );
    assert!(world.contacts.is_empty());
    assert_eq!(world.paths.resources.owner_count(owner), 0);
    let replacement = world.actor(89);
    assert_eq!(replacement, owner);
    assert_ne!(world.objects.lifetime_id(replacement).unwrap(), lifetime);
}
