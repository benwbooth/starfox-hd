//! Complete installed ballistic effect. Path and callback phases are driven
//! explicitly; ordinary velocity is zero and coordinate modes stay disabled.
use super::super::collision_surface::SurfaceMode;
use super::super::{
    authored_paths, path_motion::PublishedPlayerMotion, Angle, Behavior, ObjectKind, ShapeId,
    Vector3,
};
use super::effect_tests::callbacks;
use super::tests::{setup, world};
use super::*;

#[test]
fn ballistic_effect_uses_published_position_exact_arc_delayed_collision_and_ground_tail() {
    const ARC: [i16; 14] = [-50, -40, -32, -24, -18, -12, -4, 4, 12, 18, 24, 32, 40, 50];
    let catalog = authored_paths::catalog();
    for difference in [
        i16::MIN,
        -2049,
        -129,
        -17,
        -1,
        0,
        1,
        17,
        129,
        2049,
        i16::MAX,
    ] {
        let (mut runtime, mut objects, owner, mut random) = setup();
        let actor = objects.get_mut(owner).unwrap();
        actor.base.path = Some(authored_paths::SURFACE_LIMITED_BALLISTIC_EFFECT);
        actor.base.position = Vector3 {
            x: i16::MAX,
            y: -1023,
            z: i16::MIN,
        };
        actor.base.yaw = Angle::from_units(249);
        actor.base.hit_points = 100;
        actor.base.attack_power = 4;
        actor.extension.path_state.motion_phase = 0xA500;
        let initial = actor.base.position;
        let z_difference = difference.wrapping_neg();
        let published = Vector3 {
            x: initial.x.wrapping_sub(difference),
            y: 777,
            z: initial.z.wrapping_sub(z_difference),
        };
        // Distinct divisions truncate toward zero in the source. Do not
        // replace -(q - q/8) with a single scaled division.
        let drift = |difference: i16| {
            let q = difference / 16;
            (q - q / 8).wrapping_neg()
        };
        let expected_drift = Vector3 {
            x: drift(difference),
            y: 0,
            z: drift(z_difference),
        };
        let original_random = random;
        let mut inputs = world(&mut random);
        inputs.published_motion = Some(PublishedPlayerMotion {
            position: published,
            ..Default::default()
        });
        inputs.surface_mode = Some(SurfaceMode { flags: 0 });
        let mut expected_position = initial;
        for visit in 1..=36u8 {
            assert_eq!(
                runtime
                    .enter_program(&catalog, &mut objects, owner, &mut inputs, 80)
                    .unwrap()
                    .step,
                ControlStep::Movement
            );
            if (2..=15).contains(&visit) {
                expected_position.y = expected_position
                    .y
                    .wrapping_add(ARC[usize::from(visit - 2)]);
            } else if visit >= 16 {
                expected_position.y = expected_position.y.wrapping_add(50);
            }
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.position, expected_position);
            assert_eq!(actor.base.flags.visible, visit != 1);
            if visit >= 2 {
                assert_eq!(actor.extension.relative_position, expected_drift);
                assert_eq!(
                    actor.extension.path_state.script_value,
                    (z_difference / 16) as u16
                );
            }
            assert_eq!(
                callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
                if visit == 6 { 2 } else { 1 }
            );
            if visit >= 2 {
                expected_position.x = expected_position.x.wrapping_add(expected_drift.x);
                expected_position.z = expected_position.z.wrapping_add(expected_drift.z);
            }
            let actor = objects.get(owner).unwrap();
            assert_eq!(actor.base.position, expected_position);
            assert_eq!(
                actor.base.yaw.units(),
                249u8.wrapping_add(16u8.wrapping_mul(visit))
            );
            assert_eq!(actor.base.flags.collision_disabled, visit < 6);
            assert_eq!(actor.base.hit_points, 100);
            assert!(!actor.base.flags.remove_after_tick);
            assert_eq!(
                actor
                    .extension
                    .path_state
                    .triggers
                    .entries(&runtime.resources, owner)
                    .unwrap()
                    .len(),
                if (2..7).contains(&visit) { 2 } else { 1 }
            );
            let phase = if visit == 1 || visit == 15 {
                0
            } else if visit >= 16 {
                50
            } else {
                visit - 1
            };
            assert_eq!(
                actor.extension.path_state.motion_phase,
                0xA500 | u16::from(phase)
            );
        }
        assert_eq!(expected_position.y, 27);
        assert_eq!(
            runtime
                .enter_program(&catalog, &mut objects, owner, &mut inputs, 2)
                .unwrap()
                .step,
            ControlStep::MovementTail
        );
        let actor = objects.get(owner).unwrap();
        expected_position.y = 0;
        assert_eq!(actor.base.position, expected_position);
        assert_eq!(actor.base.hit_points, 0);
        assert_eq!(actor.base.attack_power, 4);
        assert!(actor.base.flags.suppress_death_effects);
        assert!(!actor.base.flags.remove_after_tick);
        assert_eq!(inputs.random, &original_random);
    }
}

#[test]
fn ballistic_surface_exit_precedes_horizontal_drift_and_does_not_snap_to_ground() {
    let catalog = authored_paths::catalog();
    let (mut runtime, mut objects, owner, mut random) = setup();
    let mut surface = Object::new(
        ObjectKind::Enemy,
        ShapeId::from_catalog_index(156),
        Behavior::FollowPath,
    );
    surface.base.position.y = -600;
    surface.base.contacts.first_strategy_visit = false;
    let support = objects.allocate(surface).unwrap();
    let actor = objects.get_mut(owner).unwrap();
    actor.base.path = Some(authored_paths::SURFACE_LIMITED_BALLISTIC_EFFECT);
    actor.base.position.y = -1003;
    actor.base.hit_points = 100;
    actor.extension.relative_position = Vector3 {
        x: 333,
        y: 999,
        z: -444,
    };
    actor.extension.surface_contact.group = 77;
    let original_position = actor.base.position;
    let mut inputs = world(&mut random);
    inputs.surface_mode = Some(SurfaceMode { flags: 0 });
    // This exit occurs during the first hidden visit, before any published
    // player-position import or collision-enable callback is required.
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 3)
            .unwrap()
            .step,
        ControlStep::Movement
    );
    assert_eq!(
        callbacks(&mut runtime, &catalog, &mut objects, owner, &mut inputs),
        1
    );
    let actor = objects.get(owner).unwrap();
    assert_eq!(actor.base.position, original_position);
    assert_eq!(actor.base.yaw.units(), 16);
    assert_eq!(
        actor.extension.surface_contact.supporting_object,
        Some(support)
    );
    assert_eq!(actor.extension.surface_contact.group, 77);
    assert_eq!(actor.extension.surface_contact.flags, 6);
    assert_eq!(
        runtime
            .enter_program(&catalog, &mut objects, owner, &mut inputs, 1)
            .unwrap()
            .step,
        ControlStep::MovementTail
    );
    let actor = objects.get(owner).unwrap();
    assert_eq!(actor.base.position, original_position);
    assert_eq!(actor.base.hit_points, 0);
    assert!(actor.base.flags.suppress_death_effects);
    assert!(!actor.base.flags.remove_after_tick);
}
