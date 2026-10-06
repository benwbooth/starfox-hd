use super::*;
use crate::collision_pass::ExclusionGroups;
use crate::path_program::{PathCatalog, SelectedAuxiliaryState};
use crate::scene_path_world::PlayerPathRecords;
use crate::scene_strategy::{SceneActors, SceneCallbacks, SceneError, SceneExecution};
use crate::strategy_schedule::StrategyCompletion;
use crate::weapon_dispatch::WeaponState;
use crate::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Callbacks;
impl SceneCallbacks for Callbacks {
    type Error = ();
    fn assigned(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<StrategyCompletion, ()> {
        panic!("unexpected strategy")
    }
    fn death_override(
        _: &mut SceneActors<'_, Self>,
        _: ObjectId,
    ) -> Result<Option<StrategyCompletion>, ()> {
        panic!("unexpected death")
    }
    fn resume_map_on_death(_: &mut SceneActors<'_, Self>, _: ObjectId) -> Result<(), ()> {
        panic!("unexpected map")
    }
}

struct Scene {
    objects: ObjectStore,
    world: ScenePathWorld,
    execution: SceneExecution,
    owner: ObjectId,
    proxy: ObjectId,
}
impl Scene {
    fn new(mode: u8) -> Self {
        let mut objects = ObjectStore::new();
        let owner = objects
            .allocate(Object::new(
                ObjectKind::Player,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let proxy = objects
            .allocate(Object::new(
                ObjectKind::Effect,
                ShapeId::EMPTY,
                Behavior::Unassigned,
            ))
            .unwrap();
        let mut world = ScenePathWorld::new(RandomState::new([1, 3, 5, 7]));
        world.weapons = Some(WeaponState {
            fallback: Some(proxy),
            published_pitch: Some(Angle::HALF_TURN),
            ..Default::default()
        });
        world
            .bind_player(
                &objects,
                owner,
                PlayerPathRecords {
                    auxiliary: Some(SelectedAuxiliaryState {
                        mode,
                        action_flags: 0,
                        stored_world_position: Vector3::default(),
                        stored_rotation: Default::default(),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
        let mut execution = SceneExecution::default();
        execution.paths.runtime.steering.unchanged_axes = 37;
        Self {
            objects,
            world,
            execution,
            owner,
            proxy,
        }
    }
    fn visit(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 100,
        }
        .publish_player_weapon_aim(self.owner)
    }
    fn retain(&mut self) -> Result<(), SceneError<()>> {
        SceneActors {
            objects: &mut self.objects,
            world: &mut self.world,
            execution: &mut self.execution,
            catalog: &PathCatalog::new(vec![]).unwrap(),
            callbacks: &mut Callbacks,
            statement_budget: 100,
        }
        .retain_player_weapon_aim(self.owner)
    }
    fn prepare_retained(&mut self, motion: u16) {
        let record = self.world.player_mut(&self.objects, self.owner).unwrap();
        record.yaw_motion = Some(motion);
        record.rapid_aim = Some(crate::player_rapid::RapidAim {
            roll_step: Angle::from_units(97),
            retained_aim: Vector3::default(),
        });
    }
    fn target(&mut self, y: i16) -> ObjectId {
        let mut target = Object::new(ObjectKind::Enemy, ShapeId::EMPTY, Behavior::Unassigned);
        target.base.position = Vector3 { x: 0, y, z: 1000 };
        target.base.velocity = Vector3 {
            x: 32760,
            y: -20000,
            z: 17000,
        };
        target.base.flags.general_search_eligible = true;
        target.base.contacts.exclusion_groups = ExclusionGroups::PATH_SPAWN;
        self.objects.allocate(target).unwrap()
    }
    fn pitch(&self) -> Angle {
        self.world.weapons.unwrap().published_pitch.unwrap()
    }
}

#[test]
fn all_fine_pitch_words_preserve_two_signed_rounds_and_source_clamp() {
    // Magnitudes implement truncation independently of native signed '/'.
    // The final byte and conditional comparisons reproduce the source clamp.
    for fine in 0..=u16::MAX {
        let signed = i32::from(fine as i16);
        let half_magnitude = signed.abs() >> 1;
        let scaled = (half_magnitude + (half_magnitude >> 1)) * signed.signum();
        let coarse = (scaled >> 8) as u8;
        let shifted = coarse.wrapping_add(25);
        let expected = if shifted & 0x80 != 0 {
            (-25i8) as u8
        } else if shifted.wrapping_sub(50) & 0x80 == 0 {
            25
        } else {
            coarse
        };
        assert_eq!(walker_pitch(fine).units(), expected, "fine {fine}");
    }
}

#[test]
fn every_non_walker_mode_copies_full_pitch_without_target_or_proxy_inputs() {
    let mut scene = Scene::new(0);
    scene.world.weapons.as_mut().unwrap().fallback = None;
    for mode in 0..=u8::MAX {
        if mode & 0xF0 == 0x20 {
            continue;
        }
        scene
            .world
            .player_mut(&scene.objects, scene.owner)
            .unwrap()
            .auxiliary
            .as_mut()
            .unwrap()
            .mode = mode;
        for pitch in 0..=u8::MAX {
            scene.objects.get_mut(scene.owner).unwrap().base.pitch = Angle::from_units(pitch);
            scene.visit().unwrap();
            assert_eq!(scene.pitch().units(), pitch);
            assert_eq!(scene.execution.paths.runtime.steering.unchanged_axes, 37);
        }
    }
}

#[test]
fn no_candidate_does_not_read_proxy_or_clear_shared_facing_result() {
    let mut scene = Scene::new(0x2F);
    scene.world.weapons.as_mut().unwrap().fallback = None;
    scene.objects.get_mut(scene.owner).unwrap().base.pitch = Angle::from_units(99);
    scene.visit().unwrap();
    assert_eq!(scene.pitch(), Angle::ZERO);
    assert_eq!(scene.execution.paths.runtime.steering.unchanged_axes, 37);
}

#[test]
fn proxy_uses_candidate_position_not_velocity_prediction_or_rotation() {
    for (height, expected) in [(-1000, -25i8), (-100, -6), (0, 0), (100, 5), (1000, 25)] {
        let mut scene = Scene::new(0x20);
        let target = scene.target(height);
        let target_before = scene.objects.get(target).unwrap().clone();
        let mut proxy_expected = scene.objects.get(scene.proxy).unwrap().clone();
        proxy_expected.base.position = target_before.base.position;
        // Neither live player selection nor retained aim is this fresh scan.
        scene.world.primary_player = Some(scene.proxy);
        scene.visit().unwrap();
        assert_eq!(scene.objects.get(scene.proxy).unwrap(), &proxy_expected);
        assert_eq!(scene.objects.get(target).unwrap(), &target_before);
        assert_eq!(scene.pitch().units() as i8, expected, "height {height}");
        assert_eq!(scene.execution.paths.runtime.steering.unchanged_axes, 0);
        assert!(scene
            .world
            .player(&scene.objects, scene.owner)
            .unwrap()
            .rapid_aim
            .is_none());
    }
}

#[test]
fn target_proxy_alias_retains_wrapping_prediction_and_owner_alias_reloads_origin() {
    let mut scene = Scene::new(0x20);
    let target = scene.target(100);
    scene.world.weapons.as_mut().unwrap().fallback = Some(target);
    scene.visit().unwrap();
    // Original position + four velocity steps, all wrapping signed words.
    let predicted = Vector3 {
        x: -32,
        y: -14364,
        z: 3464,
    };
    assert_eq!(scene.objects.get(target).unwrap().base.position, predicted);
    assert_eq!(
        scene.pitch(),
        walker_pitch(sf2_atan16(
            predicted.y,
            sf2_xz_angle_distance(predicted.x, predicted.z)
        ))
    );
    let mut scene = Scene::new(0x20);
    let target = scene.target(100);
    scene.world.weapons.as_mut().unwrap().fallback = Some(scene.owner);
    scene.visit().unwrap();
    assert_eq!(
        scene.objects.get(scene.owner).unwrap().base.position,
        scene.objects.get(target).unwrap().base.position
    );
    // Zero/zero is the source arctangent's quarter-turn, then clamped.
    assert_eq!(scene.pitch(), Angle::from_units(25));
}

#[test]
fn faults_preserve_early_zero_publication_and_cannot_replay_the_visit() {
    let mut scene = Scene::new(0x20);
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .auxiliary = None;
    assert_eq!(
        scene.visit(),
        Err(SceneError::WeaponAim(WeaponAimError::World(
            WorldInputError::MissingAuxiliary(scene.owner)
        )))
    );
    assert_eq!(scene.pitch(), Angle::ZERO);
    assert!(scene.execution.is_faulted());
    assert_eq!(scene.visit(), Err(SceneError::Faulted));

    let mut scene = Scene::new(0x20);
    scene.target(100);
    scene.world.weapons.as_mut().unwrap().fallback = None;
    assert_eq!(
        scene.visit(),
        Err(SceneError::WeaponAim(WeaponAimError::MissingProxy))
    );
    assert_eq!(scene.pitch(), Angle::ZERO);
    assert_eq!(scene.execution.paths.runtime.steering.unchanged_axes, 37);
    let mut scene = Scene::new(0x20);
    scene.target(100);
    scene.objects.remove(scene.proxy);
    assert_eq!(
        scene.visit(),
        Err(SceneError::WeaponAim(WeaponAimError::MissingActor(
            scene.proxy
        )))
    );
    assert_eq!(scene.pitch(), Angle::ZERO);
}

fn reference_forward(origin: Vector3, pitch: u8, yaw: u8) -> Vector3 {
    // All operands here are at most 75: signed integer division implements
    // the source's magnitude multiply with a separately restored sign.
    use sf_core::snes_trig::{COSTAB, SINTAB};
    let y = 75 * i16::from(SINTAB[pitch as usize]) / 128;
    let forward = 75 * i16::from(COSTAB[pitch as usize]) / 128;
    let angle = yaw.wrapping_neg() as usize;
    let x = forward * i16::from(SINTAB[angle]) / 128;
    let z = forward * i16::from(COSTAB[angle]) / 128;
    Vector3 {
        x: origin.x.wrapping_add(x * 128),
        y: origin.y.wrapping_add(y * 128),
        z: origin.z.wrapping_add(z * 128),
    }
}

#[test]
fn retained_point_exhausts_pitch_yaw_and_preserves_proxy_roll_and_other_records() {
    let mut scene = Scene::new(0x10);
    scene.prepare_retained(0xF3AC);
    let origin = Vector3 {
        x: 32001,
        y: -32001,
        z: 32760,
    };
    scene.objects.get_mut(scene.owner).unwrap().base.position = origin;
    scene.world.primary_player = Some(scene.proxy);
    // No fixed-view object is consumed by this routine: its initial fixed
    // identity assignment is overwritten shared work, not the origin.
    assert!(scene.world.fixed_players[0].is_none());
    for pitch in 0..=u8::MAX {
        scene.world.weapons.as_mut().unwrap().published_pitch = Some(Angle::from_units(pitch));
        for yaw in 0..=u8::MAX {
            scene.objects.get_mut(scene.owner).unwrap().base.yaw = Angle::from_units(yaw);
            scene.objects.get_mut(scene.proxy).unwrap().base.roll = Angle::from_units(!pitch);
            scene.retain().unwrap();
            let led_yaw = yaw.wrapping_add(0xE6);
            let expected = reference_forward(origin, pitch, led_yaw);
            let proxy = scene.objects.get(scene.proxy).unwrap();
            assert_eq!(proxy.base.position, expected);
            assert_eq!(proxy.base.pitch.units(), pitch);
            assert_eq!(proxy.base.yaw.units(), led_yaw);
            assert_eq!(proxy.base.roll.units(), !pitch);
            let record = scene.world.player(&scene.objects, scene.owner).unwrap();
            assert_eq!(record.rapid_aim.unwrap().retained_aim, expected);
            assert_eq!(record.rapid_aim.unwrap().roll_step.units(), 97);
            assert_eq!(
                scene.objects.get(scene.owner).unwrap().base.position,
                origin
            );
            assert_eq!(scene.execution.paths.runtime.steering.unchanged_axes, 37);
        }
    }
}

#[test]
fn retained_yaw_lead_reads_high_byte_before_doubling_for_every_motion_word() {
    let mut scene = Scene::new(0x10);
    scene.prepare_retained(0);
    scene.objects.get_mut(scene.owner).unwrap().base.yaw = Angle::from_units(41);
    scene.world.weapons.as_mut().unwrap().published_pitch = Some(Angle::from_units(19));
    for motion in 0..=u16::MAX {
        scene
            .world
            .player_mut(&scene.objects, scene.owner)
            .unwrap()
            .yaw_motion = Some(motion);
        scene.retain().unwrap();
        let yaw = ((41 + u32::from(motion / 256) * 2) % 256) as u8;
        assert_eq!(
            scene.objects.get(scene.proxy).unwrap().base.yaw.units(),
            yaw
        );
        assert_eq!(
            scene.objects.get(scene.proxy).unwrap().base.position,
            reference_forward(Vector3::default(), 19, yaw)
        );
    }
}

#[test]
fn retained_proxy_can_alias_owner_without_reapplying_lead_or_using_old_pitch() {
    let mut scene = Scene::new(0x10);
    scene.prepare_retained(0x8101);
    scene.world.weapons.as_mut().unwrap().fallback = Some(scene.owner);
    scene.world.weapons.as_mut().unwrap().published_pitch = Some(Angle::from_units(11));
    scene.objects.get_mut(scene.owner).unwrap().base.yaw = Angle::from_units(39);
    scene.objects.get_mut(scene.owner).unwrap().base.position = Vector3 {
        x: -32700,
        y: 32600,
        z: 32000,
    };
    let origin = scene.objects.get(scene.owner).unwrap().base.position;
    scene.retain().unwrap();
    let expected = reference_forward(origin, 11, 41);
    let actor = scene.objects.get(scene.owner).unwrap();
    assert_eq!(actor.base.position, expected);
    assert_eq!(actor.base.yaw.units(), 41);
    assert_eq!(actor.base.pitch.units(), 11);
    assert_eq!(
        scene
            .world
            .player(&scene.objects, scene.owner)
            .unwrap()
            .rapid_aim
            .unwrap()
            .retained_aim,
        expected
    );
}

#[test]
fn retained_faults_keep_each_preceding_publication_and_latch_partial_visits() {
    let mut scene = Scene::new(0x10);
    scene.prepare_retained(0);
    scene.objects.get_mut(scene.owner).unwrap().base.yaw = Angle::from_units(55);
    scene.world.weapons.as_mut().unwrap().published_pitch = None;
    assert_eq!(
        scene.retain(),
        Err(SceneError::WeaponAim(WeaponAimError::MissingPublishedPitch))
    );
    assert_eq!(scene.objects.get(scene.proxy).unwrap().base.yaw.units(), 55);
    assert_eq!(
        scene.objects.get(scene.proxy).unwrap().base.pitch,
        Angle::ZERO
    );
    assert!(scene.execution.is_faulted());
    assert_eq!(scene.retain(), Err(SceneError::Faulted));

    let mut scene = Scene::new(0x10);
    assert_eq!(
        scene.retain(),
        Err(SceneError::WeaponAim(WeaponAimError::MissingYawMotion(
            scene.owner
        )))
    );
    assert_eq!(
        scene.objects.get(scene.proxy).unwrap().base.pitch,
        Angle::HALF_TURN
    );
    assert_eq!(
        scene.objects.get(scene.proxy).unwrap().base.position,
        Vector3::default()
    );

    let mut scene = Scene::new(0x10);
    scene
        .world
        .player_mut(&scene.objects, scene.owner)
        .unwrap()
        .yaw_motion = Some(0);
    assert_eq!(
        scene.retain(),
        Err(SceneError::WeaponAim(WeaponAimError::MissingRetainedAim(
            scene.owner
        )))
    );
    assert_eq!(
        scene.objects.get(scene.proxy).unwrap().base.position,
        Vector3 {
            x: 0,
            y: 0,
            z: -9344
        }
    );
}
