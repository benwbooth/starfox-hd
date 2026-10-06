use super::*;
use crate::path_program::{ActionGate, PathCatalog, SelectedAuxiliaryState};
use crate::scene_contact::PlayerContactControl;
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::weapon_dispatch::WeaponState;
use crate::{Angle, Buttons, ObjectSpawnDefaults, RandomState, Vector3};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected external strategy in charged-fire test")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death dispatch in charged-fire test")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation in charged-fire test")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
    owner: ObjectId,
}

impl Scene {
    fn new() -> Self {
        let mut objects = ObjectStore::new();
        let mut player = Object::new(
            ObjectKind::Player,
            ShapeId::TITLE_CRAFT,
            Behavior::Unassigned,
        );
        player.base.hit_points = 30;
        player.base.position = Vector3 {
            x: 123,
            y: -42,
            z: 311,
        };
        player.base.pitch = Angle::from_units(27);
        player.base.yaw = Angle::from_units(193);
        player.base.roll = Angle::from_units(63);
        let owner = objects.allocate(player).unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([23, 71, 127, 219]));
        world.primary_player = Some(owner);
        world.action_gate = Some(ActionGate::default());
        world.active_charge_threshold = Some(25);
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 23,
            run_when_paused: false,
        });
        world.weapons = Some(WeaponState {
            published_pitch: Some(Angle::from_units(241)),
            ..Default::default()
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    contact: Some(PlayerContactControl::default()),
                    charge: Some(PlayerCharge::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x10,
                        action_flags: 1,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Default::default(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        Self {
            objects,
            world,
            execution: SceneExecution::default(),
            catalog: authored_paths::catalog(),
            callbacks: Callbacks,
            owner,
        }
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 128,
        }
    }
    fn visit(&mut self, held: bool, pressed: bool) -> Result<(), SceneError<&'static str>> {
        let owner = self.owner;
        self.host().advance_player_charge(
            owner,
            InputState {
                held: Buttons::from_bits(if held { Button::B as u16 } else { 0 }),
                pressed: Buttons::from_bits(if pressed { Button::B as u16 } else { 0 }),
            },
        )
    }
    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn charge(&mut self) -> &mut PlayerCharge {
        self.record().charge.as_mut().unwrap()
    }
    fn effect(&self) -> Option<ObjectId> {
        path_relationships::find_direct_child(&self.objects, self.owner, EFFECT_NUMBER).unwrap()
    }
    fn events(&mut self) -> Vec<SoundEvent> {
        self.world
            .audio
            .take_events()
            .into_iter()
            .flatten()
            .collect()
    }
    fn fill_pool(&mut self) {
        while self.objects.len() != OBJECT_CAPACITY {
            self.objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
    }
}

fn authored(id: u8, side: PlayerTarget) -> SoundEvent {
    SoundEvent::Authored(AuthoredCue::new(id, 0, side))
}

#[test]
fn remapped_controller_edges_charge_and_release_a_real_projectile_through_scene_services() {
    let mut scene = Scene::new();
    scene.world.player_input_settings = Some(crate::player_input::PlayerInputSettings {
        flight_style: 128,
        button_layout: 255,
    });
    scene.record().injected_input = Some(InputState::default());
    scene.world.controller_inputs[0] = Some(InputState::default());
    let owner = scene.owner;
    for visit in 0..=18 {
        scene.world.controller_inputs[0]
            .as_mut()
            .unwrap()
            .sample(Buttons::from_bits(Button::Y as u16));
        let processed = scene.host().prepare_player_input(owner).unwrap();
        assert!(processed.held.contains(Button::B));
        assert_eq!(processed.pressed.contains(Button::B), visit == 0);
        scene
            .host()
            .advance_player_charge(owner, processed)
            .unwrap();
        assert_eq!(scene.charge().progress, (visit * 384).min(25 * 256));
    }
    let orb = scene.effect().expect("real numbered charge effect");
    scene.world.controller_inputs[0]
        .as_mut()
        .unwrap()
        .sample(Buttons::default());
    let released = scene.host().prepare_player_input(owner).unwrap();
    assert_eq!(released, InputState::default());
    scene.host().advance_player_charge(owner, released).unwrap();
    assert_eq!(scene.charge().progress, 12 * 256);
    assert!(scene.objects.get(orb).unwrap().base.flags.remove_after_tick);
    let shots: Vec<_> = scene
        .objects
        .active_ids()
        .iter()
        .copied()
        .filter(|id| *id != owner && *id != orb)
        .collect();
    assert_eq!(shots.len(), 1);
    assert_eq!(
        scene.objects.get(shots[0]).unwrap().base.path,
        Some(authored_paths::AIMED_IMPACT_PROJECTILE)
    );
    assert_eq!(
        scene.objects.get(shots[0]).unwrap().base.shape,
        ShapeId::PLAYER_CHARGED_LASER_LAUNCH
    );
    assert_eq!(
        scene.world.controller_inputs[0],
        Some(InputState::default())
    );
    assert!(!scene.execution.is_faulted());
}

#[test]
fn processed_press_does_not_charge_and_full_high_byte_comparison_ignores_fraction() {
    let mut scene = Scene::new();
    scene.charge().control = 0x0B;
    scene.visit(true, true).unwrap();
    assert_eq!((scene.charge().progress, scene.charge().control), (0, 0x0B));
    scene.visit(true, false).unwrap();
    assert_eq!(
        (scene.charge().progress, scene.charge().control),
        (384, 0x8B)
    );
    scene.visit(true, false).unwrap();
    assert_eq!(scene.charge().progress, 768);
    scene.charge().progress = (25 << 8) | 255;
    scene.visit(true, false).unwrap();
    assert_eq!(scene.charge().progress, (25 << 8) | 255);
    assert_eq!(scene.events(), [authored(49, PlayerTarget::Primary)]);
    assert_eq!(
        scene.charge().path_input(),
        SelectedChargeInput {
            level: 25,
            linked_mode: false
        }
    );
}

#[test]
fn all_fine_progress_values_use_word_wrapping_then_unsigned_threshold_clamp() {
    let mut scene = Scene::new();
    ensure_effect(&mut scene.objects, &scene.world, scene.owner).unwrap();
    // A single real effect avoids allocation noise, while every test calls
    // the public service rather than a second copy of its arithmetic helper.
    for progress in 0..=u16::MAX {
        *scene.charge() = PlayerCharge {
            progress,
            control: 0x2D,
            ..Default::default()
        };
        scene.visit(true, false).unwrap();
        let sum = ((u32::from(progress) + 384) % 65536) as u16;
        let at_limit = progress / 256 == 25;
        let expected = if at_limit {
            progress
        } else if sum < 6400 {
            sum
        } else {
            6400
        };
        assert_eq!(scene.charge().progress, expected, "progress {progress}");
        assert_eq!(scene.charge().control, 0xAD);
        let ready = !at_limit && sum >= 6400;
        assert_eq!(
            scene.events(),
            if ready {
                vec![authored(53, PlayerTarget::Primary)]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn all_control_bytes_and_signed_levels_decay_with_fraction_reset_and_no_threshold_read() {
    let mut scene = Scene::new();
    scene.world.action_gate.as_mut().unwrap().code = 1;
    scene.world.active_charge_threshold = None;
    scene.world.spawn_defaults = None;
    scene.world.weapons = None;
    scene.record().auxiliary = None;
    for control in 0..=u8::MAX {
        for level in 0..=u8::MAX {
            *scene.charge() = PlayerCharge {
                progress: (u16::from(level) << 8) | 239,
                control,
                ..Default::default()
            };
            scene.visit(true, false).unwrap();
            let signed = if level < 128 {
                i16::from(level)
            } else {
                i16::from(level) - 256
            };
            let half = if signed < 0 {
                -((-signed) >> 1)
            } else {
                signed >> 1
            };
            assert_eq!(scene.charge().progress, u16::from(half as u8) << 8);
            let flags = (control | 0x40) & !0xA0;
            assert_eq!(
                scene.charge().control,
                if half == 0 { flags & !0x40 } else { flags }
            );
            assert_eq!(
                scene.events(),
                if control & 0x20 != 0 {
                    vec![authored(244, PlayerTarget::Primary)]
                } else {
                    vec![]
                }
            );
        }
    }
}

#[test]
fn charge_effect_uses_direct_chain_global_head_insertion_and_exact_installer_fields() {
    for mode in [0x00, 0x10, 0x1F, 0x20, 0x31] {
        let mut scene = Scene::new();
        scene.record().auxiliary.as_mut().unwrap().mode = mode;
        let tail = scene
            .objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        path_relationships::attach_fresh_child(&mut scene.objects, scene.owner, tail, 7).unwrap();
        // Direct search does not depend on the wrapper's owner flag.
        scene
            .objects
            .get_mut(scene.owner)
            .unwrap()
            .extension
            .path_state
            .motion
            .refresh_child_chain = false;
        scene.charge().progress = 8 << 8;
        scene.visit(true, false).unwrap();
        let effect = scene.effect().unwrap();
        // Ordinary allocation prepended `tail`; the source effect allocator
        // inserts after that current head, not necessarily after its player.
        assert_eq!(scene.objects.active_ids(), &[tail, effect, scene.owner]);
        assert_eq!(
            scene.objects.get(tail).unwrap().base.next_sibling,
            Some(effect)
        );
        let actor = scene.objects.get(effect).unwrap();
        assert_eq!(actor.base.path, Some(authored_paths::PLAYER_CHARGE_ORB));
        assert_eq!(actor.base.behavior, Behavior::FollowPath);
        assert!(actor.extension.path_state.needs_path_initialization);
        assert_eq!(
            actor.extension.path_state.script_value,
            if mode & 0xF0 == 0x10 { 70 } else { 20 }
        );
        assert_eq!(actor.base.shape, ShapeId::EMPTY);
        assert_eq!(actor.extension.spawn_group, 255);
        assert_eq!((actor.base.hit_points, actor.base.attack_power), (1, 1));
        assert!(
            actor.base.flags.general_search_eligible
                && actor.base.flags.collision_disabled
                && actor.base.flags.remove_with_parent
        );
        assert!(actor.base.contacts.run_when_paused);
        assert!(actor.extension.path_state.motion.attached_coordinates);
        let player = scene.objects.get(scene.owner).unwrap();
        assert_eq!(
            (
                actor.base.position,
                actor.base.pitch,
                actor.base.yaw,
                actor.base.roll
            ),
            (
                player.base.position,
                player.base.pitch,
                player.base.yaw,
                player.base.roll
            )
        );
        // Even a deferred-removal effect still satisfies the source search.
        scene
            .objects
            .get_mut(effect)
            .unwrap()
            .base
            .flags
            .remove_after_tick = true;
        scene.world.spawn_defaults = None;
        scene.record().auxiliary = None;
        scene.visit(true, false).unwrap();
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(scene.effect(), Some(effect));
        assert_eq!(scene.events(), [authored(49, PlayerTarget::Primary)]);
    }
}

#[test]
fn release_launches_real_charged_mesh_resets_shared_parameters_and_keeps_walker_recoil() {
    for (mode, primary, pressed) in [
        (0x10, true, false),
        (0x21, false, false),
        (0x31, false, true),
    ] {
        let mut scene = Scene::new();
        if !primary {
            scene.world.primary_player = None;
            scene.world.secondary_player = Some(scene.owner);
        }
        scene.record().auxiliary.as_mut().unwrap().mode = mode;
        *scene.charge() = PlayerCharge {
            progress: (25 << 8) | 211,
            control: 0xAF,
            rapid_control: 0x43,
            speed_impulse: -17,
            speed_impulse_ticks: 13,
            ..Default::default()
        };
        scene
            .world
            .weapons
            .as_mut()
            .unwrap()
            .parameters
            .pitch_offset = 93;
        ensure_effect(&mut scene.objects, &scene.world, scene.owner).unwrap();
        let orb = scene.effect().unwrap();
        let random = scene.world.random;
        scene.visit(pressed, pressed).unwrap();
        assert_eq!(
            (
                scene.charge().progress,
                scene.charge().control,
                scene.charge().rapid_control
            ),
            (12 << 8, 0x5F, 5)
        );
        assert_eq!(
            (
                scene.charge().speed_impulse,
                scene.charge().speed_impulse_ticks
            ),
            if mode & 0xF0 == 0x20 {
                (-17, 13)
            } else {
                (30, 5)
            }
        );
        assert_eq!(scene.world.weapons.unwrap().parameters, Default::default());
        assert_eq!(scene.world.random, random);
        assert!(scene.objects.get(orb).unwrap().base.flags.remove_after_tick);
        let ids: Vec<_> = scene
            .objects
            .active_ids()
            .iter()
            .copied()
            .filter(|id| *id != scene.owner && *id != orb)
            .collect();
        assert_eq!(ids.len(), 1);
        let shot = scene.objects.get(ids[0]).unwrap();
        assert_eq!(
            shot.base.path,
            Some(authored_paths::AIMED_IMPACT_PROJECTILE)
        );
        assert_eq!(shot.base.shape, ShapeId::PLAYER_CHARGED_LASER_LAUNCH);
        assert_eq!(shot.base.pitch, Angle::from_units(241));
        assert_eq!(shot.base.roll, Angle::from_units(63));
        assert_eq!((shot.base.hit_points, shot.base.attack_power), (120, 10));
        assert_eq!(
            scene.events(),
            [authored(
                244,
                if primary {
                    PlayerTarget::Primary
                } else {
                    PlayerTarget::Secondary
                }
            )]
        );
        scene.visit(false, false).unwrap();
        assert_eq!(scene.objects.len(), 3);
        assert_eq!(
            (scene.charge().progress, scene.charge().control),
            (6 << 8, 0x4F)
        );
    }
}

#[test]
fn full_pool_faults_before_charge_cues_and_after_release_parameters_are_cleared() {
    let mut scene = Scene::new();
    scene.fill_pool();
    scene.world.spawn_defaults = None;
    scene.record().auxiliary = None;
    scene.charge().progress = 24 << 8;
    scene.world.weapons.as_mut().unwrap().published_pitch = None;
    assert_eq!(
        scene.visit(true, false),
        Err(SceneError::Charge(ChargeError::ObjectPoolExhausted))
    );
    assert!(scene.events().is_empty());
    assert_eq!(scene.charge().progress, 24 << 8);
    assert_eq!(scene.charge().control, 0x80);
    assert_eq!(scene.effect(), None);
    assert_eq!(scene.visit(true, false), Err(SceneError::Faulted));

    let mut scene = Scene::new();
    scene.fill_pool();
    scene.world.spawn_defaults = None;
    scene.record().auxiliary = None;
    scene.charge().progress = (25 << 8) | 179;
    scene.charge().control = 0xB7;
    scene.world.weapons.as_mut().unwrap().parameters.yaw_offset = -37;
    assert_eq!(scene.visit(false, false),
        Err(SceneError::Charge(ChargeError::ObjectPoolExhausted)));
    assert_eq!(scene.objects.len(), OBJECT_CAPACITY);
    assert_eq!(scene.charge().rapid_control, 0);
    assert_eq!(scene.charge().speed_impulse, 0);
    assert_eq!(scene.charge().progress, (25 << 8) | 179);
    assert_eq!(scene.charge().control, 0xA7);
    assert_eq!(scene.world.weapons.unwrap().parameters, Default::default());
    assert!(scene.events().is_empty());
    assert_eq!(scene.visit(false, false), Err(SceneError::Faulted));
}

#[test]
fn ignored_input_still_reads_shared_gate_and_error_latches_partial_source_mutation() {
    let mut scene = Scene::new();
    scene.record().contact.as_mut().unwrap().ignores_contacts = true;
    scene.charge().control = 0xB7;
    scene.world.action_gate = None;
    assert_eq!(
        scene.visit(true, false),
        Err(SceneError::Charge(ChargeError::World(
            WorldInputError::MissingActionGate
        )))
    );
    assert_eq!(scene.charge().control, 0x77);
    assert!(scene.execution.is_faulted());
    scene.world.action_gate = Some(ActionGate::default());
    assert_eq!(scene.visit(true, false), Err(SceneError::Faulted));
    assert_eq!(scene.charge().control, 0x77);
}

#[test]
fn effect_installer_fault_preserves_allocation_and_attachment_without_repeating_visit() {
    let mut scene = Scene::new();
    scene.charge().progress = 8 << 8;
    scene.record().auxiliary = None;
    assert_eq!(
        scene.visit(true, false),
        Err(SceneError::Charge(ChargeError::World(
            WorldInputError::MissingAuxiliary(scene.owner)
        )))
    );
    assert_eq!(scene.objects.len(), 2);
    let orb = scene.effect().unwrap();
    assert_eq!(
        scene.objects.get(orb).unwrap().base.attachment,
        Some(scene.owner)
    );
    assert_eq!(scene.objects.get(orb).unwrap().base.path, None);
    assert_eq!(scene.charge().progress, 8 << 8);
    assert_eq!(scene.charge().control, 0x80);
    assert!(scene.events().is_empty());
    assert_eq!(scene.visit(true, false), Err(SceneError::Faulted));
    assert_eq!(scene.objects.len(), 2);
}

#[test]
fn held_controller_creates_and_drives_authored_orb_then_clean_epoch_retires_it() {
    let mut scene = Scene::new();
    let mut schedule = StrategySchedule::default();
    scene.visit(true, true).unwrap();
    let mut orb = None;
    for visit in 1..=23 {
        scene.visit(true, false).unwrap();
        scene.execution.positional.begin_epoch();
        scene.host().begin_strategy_epoch(&mut schedule).unwrap();
        schedule
            .run_overlapping(&mut scene.host(), || false)
            .unwrap();
        schedule.run_remainder(&mut scene.host()).unwrap();
        scene.host().clean_epoch().unwrap();
        assert_eq!(scene.world.strategy_clock, visit);
        if let Some(effect) = scene.effect() {
            assert_eq!(*orb.get_or_insert(effect), effect);
            if visit >= 9 {
                assert_eq!(
                    scene.objects.get(effect).unwrap().extension.parent,
                    Some(scene.owner)
                );
            }
        }
    }
    let orb = orb.unwrap();
    assert_eq!(scene.charge().progress, 25 << 8);
    assert_eq!(
        scene.objects.get(orb).unwrap().base.behavior,
        Behavior::PathMovement
    );
    assert_eq!(
        scene.objects.get(orb).unwrap().base.shape,
        ShapeId::from_catalog_index(17)
    );
    assert!(scene.execution.paths.runtime.resources.owner_count(orb) >= 2);
    scene.visit(false, false).unwrap();
    assert!(scene.objects.get(orb).unwrap().base.flags.remove_after_tick);
    scene.host().clean_epoch().unwrap();
    assert!(scene.objects.get(orb).is_none());
    assert_eq!(scene.execution.paths.runtime.resources.owner_count(orb), 0);
    assert_eq!(scene.effect(), None);
    assert!(!scene.execution.is_faulted());
    assert_eq!(
        scene.events(),
        [
            authored(49, PlayerTarget::Primary),
            authored(53, PlayerTarget::Primary),
            authored(244, PlayerTarget::Primary)
        ]
    );
}
