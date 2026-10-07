use super::*;
use crate::path_invocation::{InvocationEntry, PathInvocation};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::{Angle, ObjectSpawnDefaults, RandomState, Vector3};

fn fixture() -> (ObjectStore, ScenePathWorld, ObjectId) {
    let mut objects = ObjectStore::new();
    let owner = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::from_catalog_index(2),
            Behavior::Unassigned,
        ))
        .unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.hit_points = 1;
    actor.base.position = Vector3 {
        x: -31777,
        y: 8131,
        z: 32509,
    };
    actor.base.pitch = Angle::from_units(211);
    actor.base.yaw = Angle::from_units(193);
    actor.base.roll = Angle::from_units(91);
    let mut world = ScenePathWorld::new(RandomState::new([13, 55, 177, 244]));
    world
        .bind_player(
            &objects,
            owner,
            PlayerPathRecords {
                appearance: Some(PlayerAppearance {
                    depth_control: 0x43,
                }),
                visit: Some(Default::default()),
                contact: Some(Default::default()),
                charge: Some(Default::default()),
                ..Default::default()
            },
        )
        .unwrap();
    world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 37,
        run_when_paused: false,
    });
    world.player_carry_mode = Some(1);
    (objects, world, owner)
}

#[test]
fn low_shield_arbitration_survives_surface_until_recovery_then_republishes_whole_depth() {
    for clock in 0..=255 {
        let (mut objects, mut world, owner) = fixture();
        objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
        objects
            .get_mut(owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .carry_selected_player = true;
        world.strategy_clock = clock;
        update(&mut objects, &mut world, owner, 71).unwrap();
        world.player_carry_mode = None;
        update_surface_depth(&objects, &mut world, owner).unwrap();
        objects.get_mut(owner).unwrap().extension.depth_offset = 0xFEDC;
        publish_depth(&mut objects, &world, owner).unwrap();
        assert_eq!(
            objects.get(owner).unwrap().extension.depth_offset,
            if clock & 5 == 0 { 3 } else { 0 }
        );
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .appearance
                .unwrap()
                .depth_control,
            if clock & 5 == 0 { 0x83 } else { 0x80 }
        );
        world
            .player_mut(&objects, owner)
            .unwrap()
            .contact
            .as_mut()
            .unwrap()
            .hit
            .reserve_shield = 13;
        update(&mut objects, &mut world, owner, 71).unwrap();
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .appearance
                .unwrap()
                .depth_control,
            0
        );
        world.player_carry_mode = Some(1);
        update_surface_depth(&objects, &mut world, owner).unwrap();
        publish_depth(&mut objects, &world, owner).unwrap();
        assert_eq!(objects.get(owner).unwrap().extension.depth_offset, 2);
    }
}

#[test]
fn healthy_visits_preserve_unrelated_bits_but_clear_entire_low_shield_override() {
    let (mut objects, mut world, owner) = fixture();
    world
        .player_mut(&objects, owner)
        .unwrap()
        .contact
        .as_mut()
        .unwrap()
        .hit
        .reserve_shield = 13;
    world.player_mut(&objects, owner).unwrap().charge = None;
    world.spawn_defaults = None;
    for control in 0..=255 {
        let record = world.player_mut(&objects, owner).unwrap();
        record.appearance.as_mut().unwrap().depth_control = control;
        record.visit.as_mut().unwrap().pilot_code = control;
        assert_eq!(update(&mut objects, &mut world, owner, 0), Ok(None));
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .appearance
                .unwrap()
                .depth_control,
            if control & 0x80 == 0 { control } else { 0 }
        );
        assert_eq!(
            objects.get(owner).unwrap().extension.material_set,
            Some(if control & 1 == 0 {
                EVEN_PILOT_MATERIAL
            } else {
                ODD_PILOT_MATERIAL
            })
        );
    }
}

#[test]
fn particle_gates_do_not_borrow_later_inputs_or_consume_randomness() {
    for gate in 0..6 {
        let (mut objects, mut world, owner) = fixture();
        match gate {
            0 => objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY,
            1 => objects.get_mut(owner).unwrap().base.flags.visible = false,
            2 => objects.get_mut(owner).unwrap().base.hit_points = 0,
            3 => {
                world
                    .player_mut(&objects, owner)
                    .unwrap()
                    .contact
                    .as_mut()
                    .unwrap()
                    .ignores_contacts = true
            }
            4 => {
                world
                    .player_mut(&objects, owner)
                    .unwrap()
                    .charge
                    .as_mut()
                    .unwrap()
                    .linked_mode = true
            }
            5 => world.strategy_clock = 65535,
            _ => unreachable!(),
        }
        if gate < 3 {
            world.player_mut(&objects, owner).unwrap().contact = None;
        }
        if gate < 4 {
            world.player_mut(&objects, owner).unwrap().charge = None;
        }
        world.spawn_defaults = None;
        let random = world.random;
        assert_eq!(
            emit_damage_particle(&mut objects, &world, owner, 29),
            Ok(None)
        );
        assert_eq!(objects.len(), 1);
        assert_eq!(world.random, random);
    }
}

#[test]
fn damage_particles_are_fresh_numbered_siblings_with_deferred_real_path_initialization() {
    let (mut objects, mut world, owner) = fixture();
    world
        .player_mut(&objects, owner)
        .unwrap()
        .charge
        .as_mut()
        .unwrap()
        .linked_mode = true;
    world
        .player_mut(&objects, owner)
        .unwrap()
        .charge
        .as_mut()
        .unwrap()
        .linked_muzzle_disabled = true;
    let random = world.random;
    let parent = objects.get(owner).unwrap().clone();
    let mut children = Vec::new();
    for _ in 0..9 {
        let child = emit_damage_particle(&mut objects, &world, owner, 253)
            .unwrap()
            .unwrap();
        let actor = objects.get(child).unwrap();
        assert!(!children.contains(&child));
        assert_eq!(actor.base.path, Some(authored_paths::LOCAL_JITTER_SPRITE));
        assert_eq!(actor.base.child_number, 253);
        assert_eq!(actor.base.attachment, Some(owner));
        assert_eq!(actor.extension.parent, None);
        assert_eq!(actor.base.position, parent.base.position);
        assert_eq!(actor.base.pitch, parent.base.pitch);
        assert_eq!(actor.base.yaw, parent.base.yaw);
        assert_eq!(actor.base.roll, parent.base.roll);
        assert_eq!(actor.base.shape, DAMAGE_PARTICLE_SHAPE);
        assert_eq!(actor.extension.spawn_group, 255);
        assert_eq!(actor.base.hit_points, 1);
        assert_eq!(actor.base.attack_power, 1);
        assert!(actor.extension.path_state.needs_path_initialization);
        assert!(actor.base.contacts.run_when_paused);
        assert!(actor.base.flags.collision_disabled);
        assert!(actor.base.flags.general_search_eligible);
        children.push(child);
    }
    assert_eq!(world.random, random);
    // Run the actual catalog through the scene adapter. The newborn is not
    // pre-jittered; only this separate strategy invocation consumes randomness.
    let mut invocation = PathInvocation::default();
    let child = children[0];
    invocation.begin(child, InvocationEntry::Program).unwrap();
    invocation
        .resume(&authored_paths::catalog(), &mut objects, &mut world, 256)
        .unwrap();
    assert_ne!(world.random, random);
    assert!(
        !objects
            .get(child)
            .unwrap()
            .extension
            .path_state
            .needs_path_initialization
    );
}

#[test]
fn missing_contact_and_full_pool_retain_material_but_not_a_new_appearance_write() {
    for full in [false, true] {
        let (mut objects, mut world, owner) = fixture();
        world
            .player_mut(&objects, owner)
            .unwrap()
            .visit
            .as_mut()
            .unwrap()
            .pilot_code = 7;
        if full {
            while objects.len() < OBJECT_CAPACITY {
                objects
                    .allocate(Object::new(
                        ObjectKind::Effect,
                        ShapeId::EMPTY,
                        Behavior::Unassigned,
                    ))
                    .unwrap();
            }
        } else {
            world.player_mut(&objects, owner).unwrap().contact = None;
        }
        assert_eq!(
            update(&mut objects, &mut world, owner, 0),
            Err(if full {
                AppearanceError::ObjectPoolExhausted
            } else {
                AppearanceError::World(WorldInputError::MissingPlayerContact(owner))
            })
        );
        assert_eq!(
            objects.get(owner).unwrap().extension.material_set,
            Some(ODD_PILOT_MATERIAL)
        );
        assert_eq!(
            world
                .player(&objects, owner)
                .unwrap()
                .appearance
                .unwrap()
                .depth_control,
            0x43
        );
    }
}

#[test]
fn appearance_and_carry_absence_are_not_neutral_values_and_recycled_slots_are_rejected() {
    let (mut objects, mut world, owner) = fixture();
    world.player_carry_mode = None;
    assert_eq!(
        update_surface_depth(&objects, &mut world, owner),
        Err(AppearanceError::MissingCarryMode)
    );
    world.player_mut(&objects, owner).unwrap().appearance = None;
    assert_eq!(
        publish_depth(&mut objects, &world, owner),
        Err(AppearanceError::MissingAppearance(owner))
    );
    objects.remove(owner).unwrap();
    let replacement = objects
        .allocate(Object::new(
            ObjectKind::Player,
            ShapeId::EMPTY,
            Behavior::Unassigned,
        ))
        .unwrap();
    assert_eq!(replacement, owner);
    assert_eq!(
        publish_depth(&mut objects, &world, owner),
        Err(AppearanceError::World(WorldInputError::StalePlayerRecord(
            owner
        )))
    );
}

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map")
    }
}

#[test]
fn all_scene_appearance_services_latch_errors_and_refuse_replay() {
    for operation in 0..3 {
        let (mut objects, mut world, owner) = fixture();
        world.player_mut(&objects, owner).unwrap().appearance = None;
        objects.get_mut(owner).unwrap().base.shape = ShapeId::EMPTY;
        let mut execution = SceneExecution::default();
        let mut callbacks = Callbacks;
        let catalog = authored_paths::catalog();
        let mut host = SceneActors {
            objects: &mut objects,
            world: &mut world,
            execution: &mut execution,
            callbacks: &mut callbacks,
            catalog: &catalog,
            statement_budget: 1,
        };
        let result = match operation {
            0 => host.update_player_appearance(owner, 0).map(|_| ()),
            1 => host.update_player_surface_depth(owner),
            _ => host.publish_player_depth(owner),
        };
        assert_eq!(
            result,
            Err(SceneError::PlayerAppearance(
                AppearanceError::MissingAppearance(owner)
            ))
        );
        assert!(host.execution.is_faulted());
        assert_eq!(
            host.update_player_appearance(owner, 0),
            Err(SceneError::Faulted)
        );
        assert_eq!(
            host.update_player_surface_depth(owner),
            Err(SceneError::Faulted)
        );
        assert_eq!(host.publish_player_depth(owner), Err(SceneError::Faulted));
    }
}
