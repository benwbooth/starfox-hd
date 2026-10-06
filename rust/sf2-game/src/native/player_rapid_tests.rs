use super::super::path_equipment::SelectedEquipment;
use super::*;
use crate::path_control::PlayerTarget;
use crate::path_invocation::InvocationWorld;
use crate::path_program::{ActionGate, PathCatalog, SelectedAuxiliaryState};
use crate::path_shots::{ActiveShots, ProjectileFlightOverride};
use crate::path_sound::CueListener;
use crate::player_charge::PlayerCharge;
use crate::scene_path_world::{AudioRouting, PlayerPathRecords};
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::{StrategyCompletion, StrategySchedule};
use crate::weapon_dispatch::WeaponState;
use crate::{
    authored_paths, Behavior, Buttons, Object, ObjectKind, RandomState, ShapeId, OBJECT_CAPACITY,
};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = &'static str;
    fn assigned(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<StrategyCompletion, Self::Error> {
        panic!("unexpected external strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, Self::Error> {
        panic!("unexpected death override")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), Self::Error> {
        panic!("unexpected map continuation")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    catalog: PathCatalog,
    callbacks: Callbacks,
    owner: ObjectId,
    proxy: ObjectId,
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
        // This fixture exercises a no-impact flight lifetime. The source
        // surface search must not treat the firing craft as ground support.
        player.base.flags.exclude_from_shape_footprint_search = true;
        player.base.position = Vector3 {
            x: 123,
            y: -500,
            z: 311,
        };
        let owner = objects.allocate(player).unwrap();
        let mut proxy = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
        proxy.base.hit_points = 1;
        let proxy = objects.allocate(proxy).unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([23, 71, 127, 219]));
        world.primary_player = Some(owner);
        world.action_gate = Some(ActionGate::default());
        world.scene.player_configuration = Some(0);
        world.spawn_defaults = Some(ObjectSpawnDefaults {
            group: 23,
            run_when_paused: false,
        });
        world.weapons = Some(WeaponState {
            published_pitch: Some(Angle::from_units(7)),
            fallback: Some(proxy),
            ..Default::default()
        });
        world.audio_routing = Some(AudioRouting {
            listeners: [CueListener::PrimaryPlayer, CueListener::Other],
            markers: None,
        });
        world.surface_mode = Some(crate::collision_surface::SurfaceMode { flags: 0 });
        world.impact = Some(Default::default());
        world.projectile_flight_override = Some(ProjectileFlightOverride { code: 0 });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    contact: Some(Default::default()),
                    charge: Some(PlayerCharge::default()),
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode: 0x10,
                        action_flags: 1,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Default::default(),
                    }),
                    equipment: Some(SelectedEquipment {
                        weapon_level: 1,
                        ..Default::default()
                    }),
                    rapid_aim: Some(RapidAim {
                        roll_step: Angle::from_units(17),
                        retained_aim: Vector3 {
                            x: 700,
                            y: -900,
                            z: 1000,
                        },
                    }),
                    occupancy_exempt: Some(true),
                    ..Default::default()
                },
            )
            .unwrap();
        world
            .bind_shots(&objects, owner, ActiveShots::from_count(0))
            .unwrap();
        Self {
            objects,
            world,
            execution: SceneExecution::default(),
            catalog: authored_paths::catalog(),
            callbacks: Callbacks,
            owner,
            proxy,
        }
    }
    fn host(&mut self) -> SceneActors<'_, Callbacks> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &self.catalog,
            callbacks: &mut self.callbacks,
            statement_budget: 200,
        }
    }
    fn visit(&mut self, pressed: bool) -> Result<(), SceneError<&'static str>> {
        let owner = self.owner;
        self.host().advance_player_rapid(owner, input(pressed))
    }
    fn record(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn charge(&mut self) -> &mut PlayerCharge {
        self.record().charge.as_mut().unwrap()
    }
    fn fill_pool(&mut self) {
        while self.objects.len() < OBJECT_CAPACITY {
            self.objects
                .allocate(Object::new(
                    ObjectKind::Effect,
                    ShapeId::EMPTY,
                    Behavior::Unassigned,
                ))
                .unwrap();
        }
    }
    fn spawned(&self) -> Vec<ObjectId> {
        self.objects
            .active_ids()
            .iter()
            .copied()
            .filter(|id| *id != self.owner && *id != self.proxy)
            .collect()
    }
}
fn input(pressed: bool) -> InputState {
    InputState {
        held: Buttons::from_bits(Button::B as u16),
        pressed: Buttons::from_bits(if pressed { Button::B as u16 } else { 0 }),
    }
}

#[test]
fn all_queue_bytes_use_wrapped_signed_admission_and_only_decrement_low_delay() {
    let mut scene = Scene::new();
    // Level zero makes the source helper return without a launch, but the
    // enclosing non-null test still accepts the queued request.
    scene.record().equipment.as_mut().unwrap().weapon_level = 0;
    scene.world.action_gate = None;
    scene.record().rapid_aim = None;
    for original in 0..=u8::MAX {
        for pressed in [false, true] {
            scene.charge().rapid_control = original;
            scene.visit(pressed).unwrap();
            let admitted = pressed && (original < 64 || original >= 192);
            let queued = ((u16::from(original) + if admitted { 16 } else { 0 }) % 256) as u8;
            let expected = if queued % 16 != 0 {
                queued - 1
            } else if queued != 0 {
                queued - 16 + 1
            } else {
                0
            };
            assert_eq!(
                scene.charge().rapid_control,
                expected,
                "queue {original}, press {pressed}"
            );
            assert_eq!(scene.objects.len(), 2);
        }
    }
}

#[test]
fn full_pool_distinguishes_no_weapon_levels_and_preserves_unreached_muzzle_stores() {
    let mut scene = Scene::new();
    scene.fill_pool();
    scene.record().rapid_aim = None;
    scene.world.weapons.as_mut().unwrap().fallback = None;
    for level in 0..=u8::MAX {
        scene.record().equipment.as_mut().unwrap().weapon_level = level;
        scene.charge().rapid_control = 0x10;
        scene.world.weapons.as_mut().unwrap().parameters = LaunchParameters {
            muzzle: MuzzleOffset { x: -3, y: 5, z: 71 },
            pitch_offset: 17,
            yaw_offset: -31,
            target: Some(Vector3::default()),
        };
        scene.visit(false).unwrap();
        let no_weapon = level == 0 || level >= 129;
        assert_eq!(
            scene.charge().rapid_control,
            if no_weapon { 1 } else { 0x10 }
        );
        assert_eq!(scene.world.scene.active_weapon_level, Some(level));
        assert_eq!(
            scene.world.weapons.unwrap().parameters,
            LaunchParameters {
                muzzle: if no_weapon {
                    MuzzleOffset { x: -3, y: 5, z: 71 }
                } else {
                    MuzzleOffset::default()
                },
                ..Default::default()
            }
        );
    }
}

#[test]
fn delay_and_initializer_pause_gates_preserve_read_order_and_shared_publication() {
    let mut scene = Scene::new();
    scene.charge().rapid_control = 0x35;
    scene.record().equipment = None;
    scene.world.spawn_defaults = None;
    scene.world.weapons = None;
    scene.visit(false).unwrap();
    assert_eq!(scene.charge().rapid_control, 0x34);
    scene.charge().rapid_control = 0x30;
    scene.record().equipment = Some(SelectedEquipment {
        weapon_level: 219,
        ..Default::default()
    });
    scene.world.spawn_defaults = Some(ObjectSpawnDefaults {
        group: 71,
        run_when_paused: true,
    });
    scene.visit(false).unwrap();
    assert_eq!(scene.charge().rapid_control, 0x30);
    assert_eq!(scene.world.scene.active_weapon_level, Some(219));
    assert!(scene.world.weapons.is_none());
    assert!(!scene.execution.is_faulted());
}

#[test]
fn linked_muzzle_uses_fixed_view_xz_restores_player_and_honors_override_bit() {
    for (level, disabled) in [(1, false), (2, false), (3, false), (4, false), (1, true)] {
        let mut scene = Scene::new();
        scene.record().equipment.as_mut().unwrap().weapon_level = level;
        scene.charge().linked_mode = true;
        scene.charge().linked_muzzle_disabled = disabled;
        scene.charge().rapid_control = 0x20;
        let mut view = Object::new(ObjectKind::Effect, ShapeId::EMPTY, Behavior::Unassigned);
        view.base.position = Vector3 {
            x: 30000,
            y: 12345,
            z: -30000,
        };
        let view = scene.objects.allocate(view).unwrap();
        scene.world.fixed_players[0] = Some(view);
        let original = scene.objects.get(scene.owner).unwrap().base.position;
        scene.world.action_gate = None;
        scene.visit(false).unwrap();
        assert_eq!(
            scene.objects.get(scene.owner).unwrap().base.position,
            original
        );
        assert_eq!(scene.charge().rapid_control, 0x11);
        let shots: Vec<_> = scene
            .spawned()
            .into_iter()
            .filter(|id| *id != view)
            .collect();
        assert_eq!(shots.len(), 1);
        let shot = scene.objects.get(shots[0]).unwrap();
        let distance: i16 = if disabled {
            0
        } else if level == 1 || level == 2 {
            -20
        } else {
            20
        };
        // The source's zero-angle cosine is 127/128, with truncation after
        // each pitch/yaw stage; a nominal 20-unit offset becomes 18, not 20.
        let distance = (distance * 127 / 128) * 127 / 128;
        let start = if disabled {
            original
        } else {
            Vector3 {
                x: 30000,
                y: original.y,
                z: -30000,
            }
        };
        assert_eq!(
            shot.base.position,
            Vector3 {
                z: start.z.wrapping_add(distance * 4),
                ..start
            }
        );
        assert_eq!(
            shot.base.path,
            Some(authored_paths::RAPID_IMPACT_PROJECTILE)
        );
        assert_eq!(
            shot.base.shape,
            ShapeId::from_catalog_index(match level {
                2 => 360,
                3 => 363,
                _ => 358,
            })
        );
        assert_eq!(
            shot.extension.relative_rotation.roll,
            Angle::from_units(if level == 2 || level == 3 { 17 } else { 0 })
        );
        assert_eq!(
            scene
                .world
                .shots(&scene.objects, scene.owner)
                .unwrap()
                .count(),
            0
        );
        let retained_aim = scene.record().rapid_aim.unwrap().retained_aim;
        assert_eq!(
            scene.objects.get(scene.proxy).unwrap().base.position,
            retained_aim
        );
    }
}

#[test]
fn linked_allocation_rejection_restores_pose_but_missing_input_fault_does_not_replay() {
    let mut scene = Scene::new();
    scene.world.fixed_players[0] = Some(scene.proxy);
    scene.objects.get_mut(scene.proxy).unwrap().base.position = Vector3 {
        x: -9000,
        y: 91,
        z: 7000,
    };
    scene.charge().linked_mode = true;
    scene.charge().rapid_control = 0x10;
    scene
        .world
        .bind_shots(&scene.objects, scene.owner, ActiveShots::from_count(8))
        .unwrap();
    scene.record().rapid_aim = None;
    let original = scene.objects.get(scene.owner).unwrap().base.position;
    scene.visit(false).unwrap();
    assert_eq!(
        scene.objects.get(scene.owner).unwrap().base.position,
        original
    );
    assert_eq!(scene.charge().rapid_control, 0x10);
    scene
        .world
        .bind_shots(&scene.objects, scene.owner, ActiveShots::from_count(0))
        .unwrap();
    assert_eq!(
        scene.visit(false),
        Err(SceneError::Rapid(RapidError::Launch(LaunchError::Rapid(
            crate::weapon_rapid::RapidLaunchError::MissingRetainedAim
        ))))
    );
    assert_eq!(
        scene.objects.get(scene.owner).unwrap().base.position,
        Vector3 {
            x: -9000,
            y: original.y,
            z: 7000
        }
    );
    assert_eq!(scene.visit(true), Err(SceneError::Faulted));
    assert_eq!(scene.charge().rapid_control, 0x10);
    assert_eq!(scene.objects.len(), 2);
}

#[test]
fn alternate_branch_uses_pressed_edge_action_gate_level_mask_and_published_pitch() {
    for level in [0, 1, 2, 3, 4, 129, 254, 255] {
        let mut scene = Scene::new();
        scene.record().auxiliary.as_mut().unwrap().mode = 0x21;
        scene.record().equipment.as_mut().unwrap().weapon_level = level;
        scene.record().rapid_aim = None;
        scene.charge().rapid_control = 0xF3;
        scene.world.weapons.as_mut().unwrap().fallback = None;
        scene.world.scene.active_weapon_level = Some(173);
        scene.visit(false).unwrap();
        assert_eq!(scene.objects.len(), 2);
        scene.visit(true).unwrap();
        assert_eq!(scene.charge().rapid_control, 0xF3);
        assert_eq!(scene.world.scene.active_weapon_level, Some(173));
        assert_eq!(scene.world.weapons.unwrap().parameters.pitch_offset, -4);
        if level % 4 != 0 {
            let shot = scene.objects.get(scene.spawned()[0]).unwrap();
            assert_eq!(
                shot.base.path,
                Some(authored_paths::ALTERNATE_RAPID_IMPACT_PROJECTILE)
            );
            assert_eq!(shot.base.pitch, Angle::from_units(7));
            assert_eq!(shot.base.child_number, 252);
            assert_eq!(
                shot.base.shape,
                ShapeId::from_catalog_index(if level % 4 == 1 { 25 } else { 363 })
            );
        } else {
            assert!(scene.spawned().is_empty());
        }
    }
    let mut scene = Scene::new();
    scene.record().auxiliary.as_mut().unwrap().mode = 0x20;
    scene.record().charge = None;
    scene.record().equipment = None;
    scene.world.weapons = None;
    scene.world.spawn_defaults = None;
    scene.world.action_gate = None;
    scene.visit(false).unwrap();
    scene.world.action_gate = Some(ActionGate { code: 7 });
    scene.visit(true).unwrap();
    assert!(!scene.execution.is_faulted());
}

#[test]
fn freshly_published_walker_pitch_reaches_real_alternate_projectile() {
    let mut scene = Scene::new();
    scene.record().auxiliary.as_mut().unwrap().mode = 0x20;
    // This branch does not consume retained-target coordinates.
    scene.record().rapid_aim = None;
    let origin = scene.objects.get(scene.owner).unwrap().base.position;
    let mut target = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
    target.base.position = Vector3 {
        x: origin.x,
        y: origin.y + 100,
        z: origin.z + 1000,
    };
    target.base.flags.general_search_eligible = true;
    target.base.contacts.exclusion_groups = crate::collision_pass::ExclusionGroups::PATH_SPAWN;
    let target = scene.objects.allocate(target).unwrap();
    let owner = scene.owner;
    scene.host().publish_player_weapon_aim(owner).unwrap();
    assert_eq!(
        scene.world.weapons.unwrap().published_pitch,
        Some(Angle::from_units(5))
    );
    scene.visit(true).unwrap();
    let shot = scene
        .spawned()
        .into_iter()
        .find(|id| *id != target)
        .unwrap();
    let shot = scene.objects.get(shot).unwrap();
    assert_eq!(
        shot.base.path,
        Some(authored_paths::ALTERNATE_RAPID_IMPACT_PROJECTILE)
    );
    assert_eq!(shot.base.pitch, Angle::from_units(5));
    assert_eq!(shot.base.child_number, 252);
}

#[test]
fn path_launch_observations_follow_actual_firing_owner_not_selected_or_attached_player() {
    let mut scene = Scene::new();
    let other = scene.proxy;
    scene.world.secondary_player = Some(other);
    scene
        .world
        .bind_player(
            &scene.objects,
            other,
            PlayerPathRecords {
                equipment: Some(SelectedEquipment {
                    weapon_level: 3,
                    ..Default::default()
                }),
                rapid_aim: Some(RapidAim {
                    roll_step: Angle::from_units(19),
                    retained_aim: Vector3 {
                        x: 30,
                        y: 50,
                        z: 80,
                    },
                }),
                ..Default::default()
            },
        )
        .unwrap();
    scene
        .world
        .bind_shots(&scene.objects, other, ActiveShots::from_count(6))
        .unwrap();
    scene.objects.get_mut(scene.owner).unwrap().base.attachment = Some(other);
    let input = scene
        .world
        .path_world(&scene.objects, scene.owner, PlayerTarget::Secondary)
        .unwrap();
    assert_eq!(input.selected, Some(other));
    assert_eq!(input.selected_equipment.as_ref().unwrap().weapon_level, 3);
    let caller = input.caller_weapon_inputs.unwrap();
    assert_eq!(caller.owner, scene.owner);
    assert_eq!(caller.weapon_level, Some(1));
    assert_eq!(caller.active_shots.unwrap().count(), 0);
    assert_eq!(caller.roll_step, Some(Angle::from_units(17)));
    assert_eq!(input.linked_shot_count.as_ref().unwrap().state.count(), 6);
    drop(input);
    scene.record().equipment.as_mut().unwrap().weapon_level = 2;
    scene
        .world
        .bind_shots(&scene.objects, scene.owner, ActiveShots::from_count(7))
        .unwrap();
    assert_eq!(
        scene
            .world
            .caller_weapon_inputs(&scene.objects, scene.owner)
            .unwrap()
            .weapon_level,
        Some(2)
    );
    assert_eq!(
        scene
            .world
            .caller_weapon_inputs(&scene.objects, scene.owner)
            .unwrap()
            .active_shots
            .unwrap()
            .count(),
        7
    );
}

#[test]
fn rapid_charge_cooldown_and_real_projectile_path_share_live_count_through_retirement() {
    let mut scene = Scene::new();
    scene.visit(true).unwrap();
    let shot = scene.spawned()[0];
    assert_eq!(scene.charge().rapid_control, 1);
    assert_eq!(
        scene
            .world
            .shots(&scene.objects, scene.owner)
            .unwrap()
            .count(),
        0
    );
    let mut schedule = StrategySchedule::default();
    let mut saw_count = false;
    for _ in 0..20 {
        scene.execution.positional.begin_epoch();
        scene.host().begin_strategy_epoch(&mut schedule).unwrap();
        schedule
            .run_overlapping(&mut scene.host(), || false)
            .unwrap();
        schedule.run_remainder(&mut scene.host()).unwrap();
        scene.host().clean_epoch().unwrap();
        if scene.objects.get(shot).is_none() {
            break;
        }
        saw_count = true;
        assert_eq!(
            scene
                .world
                .shots(&scene.objects, scene.owner)
                .unwrap()
                .count(),
            1
        );
        scene.visit(false).unwrap();
    }
    assert!(saw_count);
    assert!(scene.objects.get(shot).is_none());
    assert_eq!(
        scene
            .world
            .shots(&scene.objects, scene.owner)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(scene.execution.paths.runtime.resources.owner_count(shot), 0);
    scene.world.active_charge_threshold = Some(25);
    scene.charge().progress = 25 << 8;
    let owner = scene.owner;
    scene
        .host()
        .advance_player_charge(owner, InputState::default())
        .unwrap();
    assert_eq!(scene.charge().rapid_control, 5);
    assert_eq!(
        scene.objects.get(scene.spawned()[0]).unwrap().base.path,
        Some(authored_paths::AIMED_IMPACT_PROJECTILE)
    );
    for expected in (0..5).rev() {
        scene.visit(false).unwrap();
        assert_eq!(scene.charge().rapid_control, expected);
    }
    scene.visit(true).unwrap();
    assert_eq!(scene.charge().rapid_control, 1);
    assert_eq!(scene.spawned().len(), 2);
}
