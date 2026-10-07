use super::*;
use crate::path_program::{ActionGate, SelectedAuxiliaryState};
use crate::path_protection::{DeflectionProtection, LinkedEffectActivity};
use crate::player_hit_control::ShieldRecoveryRequest;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{ObjectSpawnDefaults, RandomState};

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::from_catalog_index(2),
            Behavior::Unassigned,
        ))
        .unwrap();
    objects.get_mut(owner).unwrap().base.hit_points = 1;
    let mut world = ScenePathWorld::new(RandomState::new([17, 233, 71, 142]));
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 43,
        run_when_paused: false,
    });
    world.contacts_enabled = Some(true);
    world.action_gate = Some(ActionGate { code: 0 });
    world.shield_recovery = Some(ShieldRecoveryRequest { amount: 0 });
    world.active_shield_capacity = Some(80);
    world.linked_effect_activity = Some(LinkedEffectActivity { recent_spawn: 199 });
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                visit: Some(Default::default()),
                appearance: Some(Default::default()),
                particles: Some(Default::default()),
                protection: Some(DeflectionProtection::from_control(0xE1)),
                auxiliary: Some(SelectedAuxiliaryState {
                    mode: 0,
                    action_flags: 0,
                    stored_world_position: Default::default(),
                    stored_rotation: Default::default(),
                }),
                contact: Some(Default::default()),
                charge: Some(Default::default()),
                motion: Some(Default::default()),
                roll: Some(Default::default()),
                pose: Some(Default::default()),
                flight_displacement: Some(Default::default()),
                ..Default::default()
            },
        )
        .unwrap();
    world
        .player_mut(&objects, owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .reserve_shield = 17;
    (objects, world, owner)
}

#[test]
fn protection_gates_and_existing_pending_child_skip_unobserved_inputs() {
    let (mut objects, mut world, owner) = fixture();
    let record = *world.player(&objects, owner).unwrap();
    world.release_player_bindings(owner);
    world.contacts_enabled = Some(false);
    world.spawn_defaults = None;
    assert_eq!(install_protection(&mut objects, &world, owner), Ok(None));
    world.bind_player(&objects, owner, record).unwrap();
    world.contacts_enabled = Some(true);
    for bits in [0, 0x20, 0x40, 0x60, 0x80, 0xA0, 0xC0, 0xE0] {
        world.player_mut(&objects, owner).unwrap().protection =
            Some(DeflectionProtection::from_control(bits));
        assert_eq!(install_protection(&mut objects, &world, owner), Ok(None));
    }
    world.player_mut(&objects, owner).unwrap().protection =
        Some(DeflectionProtection::from_control(1));
    let child = objects
        .allocate(Object::new(
            ObjectKind::Effect,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    path_relationships::attach_fresh_child(&mut objects, owner, child, 18).unwrap();
    objects.get_mut(child).unwrap().base.flags.remove_after_tick = true;
    let before = objects.clone();
    assert_eq!(install_protection(&mut objects, &world, owner), Ok(None));
    assert_eq!(objects, before);
}

#[test]
fn protection_birth_is_deferred_preserves_activity_and_has_no_health_or_random_gate() {
    let (mut objects, world, owner) = fixture();
    objects.get_mut(owner).unwrap().base.hit_points = 0;
    let random = world.random;
    let child = install_protection(&mut objects, &world, owner)
        .unwrap()
        .unwrap();
    let effect = objects.get(child).unwrap();
    assert_eq!(effect.base.shape, ShapeId::from_catalog_index(84));
    assert_eq!(effect.base.child_number, 18);
    assert_eq!(
        effect.base.path,
        Some(authored_paths::LINKED_PROTECTION_EFFECT)
    );
    assert_eq!(effect.base.attachment, Some(owner));
    assert_eq!(effect.extension.parent, None);
    assert!(effect.extension.path_state.needs_path_initialization);
    assert_eq!(world.linked_effect_activity.unwrap().recent_spawn, 199);
    assert_eq!(world.random, random);
    assert_eq!(install_protection(&mut objects, &world, owner), Ok(None));
}

#[test]
fn action_gate_skips_protection_but_contacts_only_skip_birth_and_not_countdown() {
    let (mut objects, mut world, owner) = fixture();
    // Scripted view skips the damage service's independent contact read.
    world.spawn_defaults.as_mut().unwrap().run_when_paused = true;
    world.action_gate = Some(ActionGate { code: 7 });
    world.contacts_enabled = None;
    world.player_mut(&objects, owner).unwrap().protection = None;
    advance(&mut objects, &mut world, owner, 42).unwrap();
    world.action_gate = Some(ActionGate { code: 0 });
    world.contacts_enabled = Some(false);
    for control in 0..=255u8 {
        for clock in 0..=255u16 {
            world.strategy_clock = clock;
            world.player_mut(&objects, owner).unwrap().protection =
                Some(DeflectionProtection::from_control(control));
            advance(&mut objects, &mut world, owner, 42).unwrap();
            assert_eq!(
                world
                    .player(&objects, owner)
                    .unwrap()
                    .protection
                    .unwrap()
                    .control(),
                if control & 31 != 0 && clock & 7 == 0 {
                    control - 1
                } else {
                    control
                }
            );
            assert_eq!(objects.len(), 1);
        }
    }
}

#[test]
fn appearance_precedes_damage_and_recovery_and_can_remain_low_for_one_visit() {
    let (mut objects, mut world, owner) = fixture();
    objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .reserve_shield = 12;
    world.shield_recovery.as_mut().unwrap().amount = 10;
    advance(&mut objects, &mut world, owner, 99).unwrap();
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.contact.unwrap().hit.reserve_shield, 22);
    assert_eq!(record.appearance.unwrap().depth_control, 0x83);
    assert_eq!(record.protection.unwrap().control(), 0xE0);
    assert_eq!(objects.get(owner).unwrap().extension.depth_offset, 3);
    assert_eq!(world.shield_recovery.unwrap().amount, 0);
    let first = objects.get(owner).unwrap().base.attachment_next.unwrap();
    assert_eq!(objects.get(first).unwrap().base.child_number, 18);
    let next = objects.get(first).unwrap().base.attachment_next.unwrap();
    assert_eq!(objects.get(next).unwrap().base.child_number, 24);
    advance(&mut objects, &mut world, owner, 99).unwrap();
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .appearance
            .unwrap()
            .depth_control,
        0
    );
    assert_eq!(objects.get(owner).unwrap().extension.depth_offset, 0);
}

#[test]
fn damage_death_still_runs_protection_depth_and_healing_without_resurrection() {
    let (mut objects, mut world, owner) = fixture();
    objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
    let record = world.player_mut(&objects, owner).unwrap();
    record.contact.as_mut().unwrap().hit.reserve_shield = 0;
    record.particles.as_mut().unwrap().flags = 0x20;
    world.shield_recovery.as_mut().unwrap().amount = 30;
    advance(&mut objects, &mut world, owner, 88).unwrap();
    assert_eq!(objects.get(owner).unwrap().base.hit_points, 0);
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .reserve_shield,
        30
    );
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .protection
            .unwrap()
            .control(),
        0xE0
    );
    assert_eq!(objects.get(owner).unwrap().extension.depth_offset, 3);
    assert_eq!(objects.len(), 4); // Flame, protection and recovery feedback.
}

#[test]
fn fatal_protection_keeps_appearance_but_precedes_countdown_depth_and_request_consumption() {
    let (mut objects, mut world, owner) = fixture();
    objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
    objects.get_mut(owner).unwrap().extension.depth_offset = 0xACED;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .reserve_shield = 12;
    world.shield_recovery.as_mut().unwrap().amount = 10;
    while objects.len() < OBJECT_CAPACITY {
        objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
    }
    assert_eq!(
        advance(&mut objects, &mut world, owner, 77),
        Err(FrameEffectsError::ObjectPoolExhausted)
    );
    let record = world.player(&objects, owner).unwrap();
    assert_eq!(record.appearance.unwrap().depth_control, 0x83);
    assert_eq!(record.protection.unwrap().control(), 0xE1);
    assert_eq!(record.contact.unwrap().hit.reserve_shield, 12);
    assert_eq!(objects.get(owner).unwrap().extension.depth_offset, 0xACED);
    assert_eq!(world.shield_recovery.unwrap().amount, 10);
    world.action_gate.as_mut().unwrap().code = 1;
    assert_eq!(
        advance(&mut objects, &mut world, owner, 77),
        Err(FrameEffectsError::Recovery(
            RecoveryError::ObjectPoolExhausted
        ))
    );
    assert_eq!(objects.get(owner).unwrap().extension.depth_offset, 3);
    assert_eq!(world.shield_recovery.unwrap().amount, 0);
    assert_eq!(
        world
            .player(&objects, owner)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .reserve_shield,
        22
    );
}

#[test]
fn composed_attachment_failure_keeps_effects_and_latches_scene_fault() {
    let (mut objects, mut world, owner) = fixture();
    world
        .player_mut(&objects, owner)
        .unwrap()
        .auxiliary
        .as_mut()
        .unwrap()
        .mode = 0x2F;
    let catalog = authored_paths::catalog();
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    assert_eq!(
        host.advance_player_frame_publication(owner, 77),
        Err(SceneError::PlayerFrameEffects(
            FrameEffectsError::Attachments(PlayerAttachmentError::MissingGround(owner))
        ))
    );
    assert!(
        path_relationships::find_direct_child(host.objects, owner, 18)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        host.world
            .player(host.objects, owner)
            .unwrap()
            .protection
            .unwrap()
            .remaining(),
        0
    );
    assert_eq!(
        host.advance_player_frame_publication(owner, 77),
        Err(SceneError::Faulted)
    );
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected callback")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death override")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map callback")
    }
}

#[test]
fn scene_failure_keeps_completed_prefix_and_cannot_be_reentered() {
    let (mut objects, mut world, owner) = fixture();
    world.action_gate = None;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .particles
        .as_mut()
        .unwrap()
        .flags = 0x20;
    let catalog = authored_paths::catalog();
    let mut execution = SceneExecution::default();
    let mut callbacks = Callbacks;
    let mut host = SceneActors {
        objects: &mut objects,
        world: &mut world,
        execution: &mut execution,
        catalog: &catalog,
        callbacks: &mut callbacks,
        statement_budget: 256,
    };
    assert_eq!(
        host.advance_player_frame_effects(owner, 71),
        Err(SceneError::PlayerFrameEffects(FrameEffectsError::World(
            WorldInputError::MissingActionGate
        )))
    );
    assert_eq!(
        host.world
            .player(host.objects, owner)
            .unwrap()
            .contact
            .unwrap()
            .hit
            .reserve_shield,
        16
    );
    let before = host.objects.clone();
    assert_eq!(
        host.advance_player_frame_effects(owner, 71),
        Err(SceneError::Faulted)
    );
    assert_eq!(*host.objects, before);
}
