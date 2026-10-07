//! Complete original engine-sound caller, with independently retained native
//! sound state and all currently source-complete player-action identities.
use super::{rom, Source, WRAM};
use sf2_game::path_runtime::PathRuntime;
use sf2_game::path_scene_state::EncounterObjectiveCounts;
use sf2_game::player_action::PlayerAction;
use sf2_game::player_engine_sound::{self, EngineSoundControl};
use sf2_game::player_storage::{self, PlayerStorageInputs};
use sf2_game::scene_path_world::{PlayerPathRecords, ScenePathWorld};
use sf2_game::{Behavior, Object, ObjectId, ObjectKind, ObjectStore, RandomState, ShapeId};

const OWNER: u16 = 0x0500;
const SLOT: u32 = 64;

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
        world.objective_counts = Some(EncounterObjectiveCounts {
            remaining_word: 1,
            ..Default::default()
        });
        world.player_carry_mode = Some(0);
        world.engine_sound_control = Some(EngineSoundControl::default());
        Self {
            objects,
            world,
            owner,
        }
    }
    fn records(&mut self) -> &mut PlayerPathRecords {
        self.world.player_mut(&self.objects, self.owner).unwrap()
    }
    fn bytes(&self) -> Vec<(u32, u8)> {
        let r = self.world.player(&self.objects, self.owner).unwrap();
        let contact = r.contact.unwrap();
        let motion = r.motion.unwrap();
        vec![
            (SLOT + 0x6AA0, r.auxiliary.unwrap().mode),
            (SLOT + 0x6B77, r.auxiliary.unwrap().action_flags),
            (
                SLOT + 0x6A72,
                0xEF | u8::from(contact.ignores_contacts) * 0x10,
            ),
            (
                SLOT + 0x6B7D,
                0x37 | u8::from(contact.hit.hold_secondary_protection) * 0x80,
            ),
            (SLOT + 0x6ADD, r.roll.unwrap().impulse as u8),
            (SLOT + 0x6AE9, motion.walker_turn_control),
            (SLOT + 0x6AEA, motion.walker_stride_control),
            (SLOT + 0x6BFF, r.visit.unwrap().pilot_code),
            (0x1E13, self.world.player_carry_mode.unwrap()),
            (
                u32::from(OWNER) + 0x21,
                0xDF | u8::from(
                    self.objects
                        .get(self.owner)
                        .unwrap()
                        .extension
                        .path_state
                        .motion
                        .carry_selected_player,
                ) * 0x20,
            ),
        ]
    }
    fn words(&self) -> Vec<(u32, u16)> {
        let r = self.world.player(&self.objects, self.owner).unwrap();
        vec![
            (SLOT + 0x6ACD, r.yaw_motion.unwrap()),
            (
                SLOT + 0x6C13,
                match r.action.unwrap().action {
                    None => 0,
                    Some(PlayerAction::TriggeredProjectile) => 0xBDDA,
                    Some(PlayerAction::ForcedRetreat) => 0xBF63,
                },
            ),
            (
                u32::from(OWNER) + 0x34,
                self.objects.get(self.owner).unwrap().base.velocity.y as u16,
            ),
            (0xD7F4, self.world.objective_counts.unwrap().remaining_word),
            (
                u32::from(OWNER) + 0x1CE4,
                self.objects
                    .get(self.owner)
                    .unwrap()
                    .extension
                    .path_state
                    .script_value,
            ),
        ]
    }
    fn seed_inputs(&self, source: &mut Source) {
        source.bus.write16(u32::from(OWNER) + 0x2B, SLOT as u16);
        for (a, v) in self.bytes() {
            source.bus.write8(WRAM + a, v);
        }
        for (a, v) in self.words() {
            source.bus.write16(WRAM + a, v);
        }
    }
    fn seed(&self, source: &mut Source) {
        self.seed_inputs(source);
        source.bus.write8(
            WRAM + 0x1CE5,
            self.world.engine_sound_control.unwrap().bits(),
        );
    }
    fn visit(&mut self, source: &mut Source) {
        source.run(0x069236, None, 0, OWNER, true);
        player_engine_sound::advance(&self.objects, &mut self.world, self.owner).unwrap();
        assert_eq!(
            source.bus.read8(WRAM + 0x1CE5),
            self.world.engine_sound_control.unwrap().bits(),
            "engine mode={:02X}",
            self.records().auxiliary.unwrap().mode
        );
        for (a, v) in self.bytes() {
            assert_eq!(source.bus.read8(WRAM + a), v, "input byte {a:04X}");
        }
        for (a, v) in self.words() {
            assert_eq!(source.bus.read16(WRAM + a), v, "input word {a:04X}");
        }
    }
}

#[test]
fn engine_sound_matches_original_all_modes_action_flags_and_protection_branches() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for branch in 0..4u8 {
        for word in 0..=u16::MAX {
            let r = f.records();
            r.auxiliary.as_mut().unwrap().mode = word as u8;
            r.auxiliary.as_mut().unwrap().action_flags = (word >> 8) as u8;
            r.contact.as_mut().unwrap().hit.hold_secondary_protection = branch & 1 != 0;
            r.contact.as_mut().unwrap().ignores_contacts = branch & 2 != 0;
            r.motion.as_mut().unwrap().walker_turn_control = word.rotate_left(3) as u8;
            r.motion.as_mut().unwrap().walker_stride_control = word.rotate_right(5) as u8;
            r.yaw_motion = Some(word.rotate_left(9));
            r.roll.as_mut().unwrap().impulse = word.rotate_left(7) as i8;
            f.world.player_carry_mode = Some((word >> 8) as u8);
            f.world.engine_sound_control =
                Some(EngineSoundControl::from_bits(word.rotate_right(3) as u8));
            f.objects
                .get_mut(f.owner)
                .unwrap()
                .extension
                .path_state
                .motion
                .carry_selected_player = word & 1 != 0;
            f.seed(&mut source);
            f.visit(&mut source);
        }
    }
}

#[test]
fn engine_sound_matches_original_every_vertical_word_and_all_pilot_speed_thresholds() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for pilot in [0, 1, 2, 3, 4, 5, 6, 255] {
        for word in 0..=u16::MAX {
            f.records().visit.as_mut().unwrap().pilot_code = pilot;
            f.objects.get_mut(f.owner).unwrap().base.velocity.y = word as i16;
            f.world.engine_sound_control = Some(EngineSoundControl::from_bits(word as u8));
            f.seed(&mut source);
            f.visit(&mut source);
        }
    }
}

#[test]
fn engine_sound_matches_original_every_yaw_word_and_roll_boundaries() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    for roll in [0, 9, -9, 10, -10, 127, -127, -128] {
        for word in 0..=u16::MAX {
            f.records().roll.as_mut().unwrap().impulse = roll;
            f.records().yaw_motion = Some(word);
            f.world.engine_sound_control = Some(EngineSoundControl::from_bits(word as u8));
            f.seed(&mut source);
            f.visit(&mut source);
        }
    }
}

#[test]
fn engine_sound_matches_original_all_walker_controls_and_previous_sound_bits() {
    let mut source = Source::new(&rom(), 0);
    let mut f = Fixture::new();
    f.records().auxiliary.as_mut().unwrap().mode = 0x31;
    for prior in [0, 0xFF, 0x80, 0x43] {
        for word in 0..=u16::MAX {
            let motion = f.records().motion.as_mut().unwrap();
            motion.walker_turn_control = word as u8;
            motion.walker_stride_control = (word >> 8) as u8;
            f.world.engine_sound_control = Some(EngineSoundControl::from_bits(prior));
            f.seed(&mut source);
            f.visit(&mut source);
        }
    }
}

#[test]
fn engine_sound_retains_independent_state_across_objective_gates_and_live_motion() {
    let mut source = Source::new(&rom(), 0);
    for seed in 0..32u16 {
        let mut f = Fixture::new();
        f.world.engine_sound_control =
            Some(EngineSoundControl::from_bits(seed.wrapping_mul(13) as u8));
        f.seed(&mut source);
        for visit in 0..256u16 {
            let word = visit.wrapping_mul(257).wrapping_add(seed.wrapping_mul(19));
            let r = f.records();
            r.auxiliary.as_mut().unwrap().mode = word as u8;
            r.auxiliary.as_mut().unwrap().action_flags = word.rotate_right(3) as u8;
            r.yaw_motion = Some(word);
            r.roll.as_mut().unwrap().impulse = (word >> 8) as i8;
            r.contact.as_mut().unwrap().ignores_contacts = visit % 7 == 0;
            r.contact.as_mut().unwrap().hit.hold_secondary_protection = visit % 13 == 0;
            r.motion.as_mut().unwrap().walker_turn_control = word.rotate_left(3) as u8;
            r.motion.as_mut().unwrap().walker_stride_control = word.rotate_right(5) as u8;
            r.action.as_mut().unwrap().action = match visit % 3 {
                0 => Some(PlayerAction::TriggeredProjectile),
                1 => Some(PlayerAction::ForcedRetreat),
                _ => None,
            };
            f.world.objective_counts.as_mut().unwrap().remaining_word =
                if visit % 5 == 0 { 0xDE00 } else { word | 1 };
            f.objects.get_mut(f.owner).unwrap().base.velocity.y = word as i16;
            f.objects
                .get_mut(f.owner)
                .unwrap()
                .extension
                .path_state
                .script_value = word;
            f.seed_inputs(&mut source);
            f.visit(&mut source);
        }
    }
}
