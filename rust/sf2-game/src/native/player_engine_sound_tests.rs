use super::*;
use crate::native::path_runtime::PathRuntime;
use crate::native::path_scene_state::EncounterObjectiveCounts;
use crate::native::player_storage::{self, PlayerStorageInputs};
use crate::native::{Behavior, Object, ObjectKind, RandomState, ShapeId};

struct Fixture {
    objects: ObjectStore,
    world: ScenePathWorld,
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
        let mut runtime = PathRuntime::default();
        player_storage::initialize(
            &mut objects,
            &mut world,
            &mut runtime,
            owner,
            PlayerStorageInputs {
                pilot_code: 0,
                reserve_shield: 20,
                score: Default::default(),
            },
        )
        .unwrap();
        world.contacts_enabled = Some(true);
        world.engine_sound_control = Some(EngineSoundControl::default());
        world.player_carry_mode = Some(0);
        Self {
            objects,
            world,
            owner,
        }
    }
    fn records(&mut self) -> &mut super::super::scene_path_world::PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn run(&mut self, previous: u8) -> u8 {
        self.world.engine_sound_control = Some(EngineSoundControl::from_bits(previous));
        advance(&self.objects, &mut self.world, self.owner).unwrap();
        self.world.engine_sound_control.unwrap().bits()
    }
}

#[test]
fn live_objective_low_byte_skips_before_sound_or_movement_dependencies() {
    let mut f = Fixture::new();
    f.world.objective_counts = Some(EncounterObjectiveCounts {
        remaining_word: 0xA500,
        ..Default::default()
    });
    f.world.engine_sound_control = None;
    f.records().auxiliary = None;
    advance(&f.objects, &mut f.world, f.owner).unwrap();
    assert_eq!(f.world.engine_sound_control, None);
    f.world.objective_counts.as_mut().unwrap().remaining_word |= 1;
    assert_eq!(
        advance(&f.objects, &mut f.world, f.owner),
        Err(EngineSoundError::MissingSoundControl)
    );
}

#[test]
fn flight_boost_precedes_brake_and_clears_stale_motion_bits() {
    let mut f = Fixture::new();
    for flags in 0..=u8::MAX {
        f.records().auxiliary.as_mut().unwrap().action_flags = flags;
        for previous in 0..=u8::MAX {
            let throttle = if flags & 0x40 != 0 {
                8
            } else if flags & 0x20 != 0 {
                12
            } else {
                4
            };
            assert_eq!(f.run(previous), (previous & 0x80) | throttle);
        }
    }
}

#[test]
fn flight_protection_and_contact_exemption_skip_unneeded_inputs() {
    let mut f = Fixture::new();
    f.records().visit = None;
    f.records().roll = None;
    f.records().yaw_motion = None;
    for protected in [false, true] {
        for exempt in [false, true] {
            if !protected && !exempt {
                continue;
            }
            let contact = f.records().contact.as_mut().unwrap();
            contact.ignores_contacts = exempt;
            contact.hit.hold_secondary_protection = protected;
            for previous in 0..=u8::MAX {
                assert_eq!(
                    f.run(previous),
                    (previous & 0x80) | if protected { 0x40 } else { 0 }
                );
            }
        }
    }
}

#[test]
fn surface_mode_uses_carry_not_flight_contact_protection() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x2F;
    f.records().contact = None;
    f.records().roll = None;
    for carry_mode in 0..=u8::MAX {
        f.world.player_carry_mode = Some(carry_mode);
        for carried in [false, true] {
            f.objects
                .get_mut(f.owner)
                .unwrap()
                .extension
                .path_state
                .motion
                .carry_selected_player = carried;
            assert_eq!(
                f.run(0xFF),
                0x80 | if carry_mode == 1 && carried { 0x40 } else { 0 }
            );
        }
    }
}

#[test]
fn walker_replaces_old_bits_and_combines_stride_and_turn_controls() {
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x3F;
    f.records().visit = None;
    f.records().roll = None;
    for turn in 0..=u8::MAX {
        for stride in 0..=u8::MAX {
            let motion = f.records().motion.as_mut().unwrap();
            motion.walker_turn_control = turn;
            motion.walker_stride_control = stride;
            let expected =
                if turn & 8 != 0 { 0x34 } else { 0 } | if stride & 0x80 != 0 { 0x2C } else { 0 };
            assert_eq!(f.run(turn ^ stride), expected);
        }
    }
    f.records().motion = None;
    f.records().contact.as_mut().unwrap().ignores_contacts = true;
    assert_eq!(f.run(0xFF), 0);
}

#[test]
fn signed_motion_bands_preserve_wrapped_minimum_and_exact_thresholds() {
    let mut f = Fixture::new();
    for (turn, expected) in [
        (0, 0),
        (512, 0),
        (513, 1),
        (768, 1),
        (769, 2),
        (960, 2),
        (961, 3),
        (-961, 3),
        (i16::MIN, 3),
    ] {
        f.records().yaw_motion = Some(turn as u16);
        assert_eq!(f.run(0), 4 | expected);
    }
    f.records().yaw_motion = Some(0);
    for (roll, expected) in [(0, 0), (9, 0), (-9, 0), (10, 3), (-10, 3), (i8::MIN, 3)] {
        f.records().roll.as_mut().unwrap().impulse = roll;
        assert_eq!(f.run(0), 4 | expected);
    }
    f.records().roll.as_mut().unwrap().impulse = 0;
    for (vertical, moving) in [
        (0, false),
        (4, false),
        (-4, false),
        (5, true),
        (-5, true),
        (i16::MIN, true),
    ] {
        f.objects.get_mut(f.owner).unwrap().base.velocity.y = vertical;
        assert_eq!(f.run(0), 4 | if moving { 0x10 } else { 0 });
    }
}

#[test]
fn missing_late_roll_does_not_publish_partial_sound_or_change_camera_counter() {
    let mut f = Fixture::new();
    f.world.engine_sound_control = Some(EngineSoundControl::from_bits(0xED));
    f.objects
        .get_mut(f.owner)
        .unwrap()
        .extension
        .path_state
        .script_value = 0x1234;
    f.records().roll = None;
    assert_eq!(
        advance(&f.objects, &mut f.world, f.owner),
        Err(EngineSoundError::MissingRoll(f.owner))
    );
    assert_eq!(f.world.engine_sound_control.unwrap().bits(), 0xED);
    assert_eq!(
        f.objects
            .get(f.owner)
            .unwrap()
            .extension
            .path_state
            .script_value,
        0x1234
    );
}
